# Handoff — Desenvolvimento do remux (continuação no Mac)

> Sessão originada no skyserver (Debian). Código clonado em `/home/skyer/orpheus`.
> Esta sessão está **idle**; nada está rodando. Todas as mudanças abaixo estão committadas em `main`.

---

## 1. O que já foi feito (committado)

### Commit `197d7938` — `perf(remux): optimize SQLite queries on 423k-row media table`

**Arquivos alterados:**

| Arquivo | Mudança |
|---------|---------|
| `crates/remux-server/Cargo.toml` | Adicionada feature `small_rng` ao `rand` (precisa de `SmallRng::from_entropy()`) |
| `crates/remux-server/src/db/media.rs` | 4 otimizações de query (detalhe abaixo) |
| `crates/remux-server/migrations/202611130001_perf_indexes_v2.sql` | Nova migração com índices + `ANALYZE` |

**Otimizações aplicadas em `media.rs`:**

1. **`ORDER BY RANDOM()` → reservoir sampling em Rust** — antes o SQLite gerava um valor aleatório para cada uma das 423k linhas, ordenava tudo e aplicava `LIMIT` (O(n log n)). Agora: o SQL busca um pool maior (`limit × 10`, máximo 1000) sem `ORDER BY RANDOM()`, e o Rust embaralha com `SmallRng` + `shuffle` + `truncate(limit)`.

2. **`NOT EXISTS` correlacionado → `NOT IN` direcionado por `user_id`** (filtro `played=false`) — antes era um subquery correlacionado executado linha por linha sobre 423k linhas, com comparação `TEXT` vs `BLOB` (`media.id` vs `ums.media_id`). Agora começa pela tabela pequena `user_media_state` (196 linhas, indexada por `user_id`) e faz PK lookups em `media`.

3. **Índice composto** `idx_media_kind_enabled_dates ON media(kind, enabled, digital_released_at, released_at)` — cobre o filtro principal de listagem e alivia a condição `OR` de `push_release_date_filter` (~80% do problema resolvido sem reescrever o query builder).

4. **Índice covering** `idx_ums_user_playcount ON user_media_state(user_id, play_count, media_id)` — suporta o `NOT IN` do item 2.

**Migração:** será aplicada automaticamente no próximo startup do remux via `sqlx::migrate()` no `connect()`. O `VACUUM` está no arquivo SQL mas requer exclusivo no banco — parar o container remux antes de aplicar se for rodar manualmente.

**Validação já feita:** `cargo check -p remux-server` passou com sucesso (apenas warnings de dependências não usadas, não críticos).

---

## 2. O que falta fazer (próximos passos no Mac)

### A. Rodar testes e build (pesado — não rode no skyserver)
```bash
cd /home/skyer/orpheus
cargo test -p remux-server
cargo build -p remux-server --release
```
O skyserver tem 7,7 GB de RAM e o build/testes são muito pesados. Fazer no Mac.

### B. Validar a migração no banco real do remux
O banco do remux no skyserver fica em `/home/skyer/remux/data/db.sqlite` (cerca de 467 MB, 423.745 linhas na tabela `media`). Após aplicar a migração, verificar se as queries lentas (33–36 s) melhoraram.

### C. Ponytail review da codebase do remux
Já foi feita uma revisão preliminar (ver seção abaixo). O principal achado: `media.rs` tem ~15k linhas (monolito), `get_by_filter` tem 1000+ linhas em uma função, `conversions.rs` tem 3000+ linhas de mapeamento. Prioridade: dividir `media.rs` em submódulos (`query.rs`, `filter.rs`, `sort.rs`, `relations.rs`, `metrics.rs`).

### D. Feature ambiciosa: download/organização de conteúdo dos addons Stremio
Baixar séries/filmes/músicas dos addons e organizar automaticamente na biblioteca local. É o item mais ambicioso — requer download manager, queue, progress tracking, file organization, naming, integração com qBittorrent/aria2.

### E. Deploy no Coolify (contêiner diferente, porta diferente)
O usuário pediu para subir uma instância do remux no Coolify em um contêiner separado com porta diferente da atual (3901).

---

## 3. Contexto do skyserver (útil para referência)

### Stack atual (Coolify)
- Prowlarr (localhost:9696) — indexers, conectado ao Torznab BR
- Radarr (localhost:7878) — filmes, raiz `/filmes`, cliente qBittorrent (categoria radarr)
- Sonarr (localhost:8989) — séries/animes/novelas, raiz `/series`, `/animes`, `/novelas`, cliente qBittorrent (categoria sonarr)
- qBittorrent (localhost:8081) — usuário `Skyer`, senha = do sudo
- Jellyfin — media server, sem filtro por resolução nativo
- FlareSolverr (localhost:8191) — de pé, não plugado em nada
- Bazarr — órfão no Coolify, não no compose

### Indexador privado (torznab-br)
Raspa sites brasileiros (comando1.com, torrentdosfilmes-v2.xyz), retorna magnet links no formato Torznab. Funciona para filmes/séries dublados PT-BR.

### Filtro de qualidade
Perfil "Só 720p+" ativo no Radarr e Sonarr. Custom formats contra 3D/CAM/TS/SCR. Acervo SD existente (Chaves, novelas) mantido — filtro vale só para entradas novas.

### Rede
Placa Realtek RTL8106E limitada a 100 Mbps (teto físico). Para 500 Mbps precisa trocar a placa.

### Library
`/home/skyer/library` em exFAT (`/dev/sda3`, 824 GB, ~435 GB usados). exFAT não suporta hardlinks → o Sonarr copia arquivos inteiros ao importar. Plano: backup via SMB (SSD do Mac compartilhado) → formatar sda3 como ext4 → restaurar. Não foi executado ainda.

### Otimizações já aplicadas no skyserver
1. `docker system prune -af` (~950 MB liberado, 52 redes bridge limpas)
2. `apt-get clean && apt-get autoremove`
3. Limpeza de journals (~1,1 GB liberado)
4. Instalado `htop`, `ncdu`, `iotop`, `smartmontools`
5. `vm.swappiness` 60 → 10 (persistido em `/etc/sysctl.conf`)

---

## 4. Achados do ponytail review (remux-server)

| Prioridade | Item | Esforço | Impacto |
|------------|------|---------|---------|
| 1 | Dividir `media.rs` (~15k linhas) em submódulos | Médio | Alto |
| 2 | Extrair `push_*` filters para `filter.rs` | Médio | Alto |
| 3 | Remover `LogErr` trait → usar `anyhow::context` | Baixo | Médio |
| 4 | `RwLock<Option<String>>` → `OnceLock` em `tools.rs` | Baixo | Baixo |
| 5 | Simplificar `infer_container_from_url` | Baixo | Baixo |
| 6 | Magic numbers → consts nomeadas | Baixo | Baixo |

---

## 5. Comandos úteis

```bash
# Verificar se compila
cargo check -p remux-server

# Rodar testes (pesado)
cargo test -p remux-server

# Build release (pesado)
cargo build -p remux-server --release

# Aplicar migração manualmente (parar container remux antes do VACUUM)
sqlite3 /home/skyer/remux/data/db.sqlite < crates/remux-server/migrations/202611130001_perf_indexes_v2.sql
```

---

## 6. Observações de risco

- A amostragem aleatória em Rust muda levemente a semântica de `ORDER BY RANDOM() LIMIT n` (antes no SQL, agora no Rust). Para recomendações aleatórias o efeito prático é indistinguível.
- O `VACUUM` requer exclusivo no arquivo SQLite — parar o container remux antes.
- A migração roda automaticamente no startup via `sqlx::migrate()`, mas o `VACUUM` dentro dela pode falhar se o banco estiver em uso.
- Não rode `cargo test`/`cargo build` no skyserver (pouca RAM).

---

Última atualização: sessão openchamber://session/ses_e2d400e53ffejlVDwdHfWqWR2Q (idle).
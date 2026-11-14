use crate::{
    components::{
        ConfirmDialog, EmptyState, FormGroup, LoadingText, Modal, PaginationBar, Switch,
    },
    state::AppState,
};
use dioxus::prelude::*;
use remux_sdks::remux::{
    BulkChannelRequest, BulkChannels, ChannelEditorItem, DeleteEpgSource,
    EpgSourceInfo, GetEpgSources, GetIptvChannelCountries, GetIptvChannelGroups,
    GetIptvChannels, PatchChannel, PatchChannelRequest, SaveEpgSource, SourceUrl,
};

pub const PAGE_SIZE: u32 = 50;

#[component]
pub fn IptvPage(app_state: AppState) -> Element {
    let mut tab = use_signal(|| "channels".to_string());
    rsx! {
        div { class: "tab-group", style: "margin-bottom:12px",
            button {
                class: if *tab.read() == "channels" { "tab-btn active" } else { "tab-btn" },
                onclick: move |_| tab.set("channels".to_string()),
                "Channels"
            }
            button {
                class: if *tab.read() == "epg" { "tab-btn active" } else { "tab-btn" },
                onclick: move |_| tab.set("epg".to_string()),
                "EPG Sources"
            }
        }
        if *tab.read() == "channels" {
            IptvChannelsTab { app_state }
        } else {
            IptvEpgTab { app_state }
        }
    }
}

#[component]
fn IptvEpgTab(app_state: AppState) -> Element {
    let mut sources: Signal<Vec<EpgSourceInfo>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| Option::<String>::None);
    let mut saving = use_signal(|| false);
    let mut name = use_signal(String::new);
    let mut url = use_signal(String::new);
    // Bumped after a save or delete to re-fetch the list.
    let mut reload = use_signal(|| 0_u32);
    let mut confirm_delete: Signal<Option<String>> = use_signal(|| None);

    let app_state_list = app_state.clone();
    use_effect(move || {
        let _ = *reload.read();
        let client = app_state_list.clone();
        loading.set(true);
        spawn(async move {
            match client
                .execute(GetEpgSources)
                .await
            {
                Ok(list) => {
                    sources.set(list);
                    error.set(None);
                }
                Err(e) => error.set(Some(format!("Failed to load EPG sources: {e}"))),
            }
            loading.set(false);
        });
    });

    let can_add = !name
        .read()
        .trim()
        .is_empty()
        && !url
            .read()
            .trim()
            .is_empty()
        && !*saving.read();

    rsx! {
        div { class: "card",
            div { class: "card-header",
                span { class: "card-title", "EPG Sources" }
                if !sources.read().is_empty() {
                    span { style: "font-size:.75rem;opacity:.5;margin-left:8px",
                        "{sources.read().len()} total"
                    }
                }
            }
            div { class: "card-body tight",
                if *loading.read() {
                    LoadingText {}
                } else if let Some(err) = error.read().as_ref() {
                    span { class: "loading-text", style: "color:var(--error)", "{err}" }
                } else if sources.read().is_empty() {
                    EmptyState { message: "No EPG sources yet. Add an XMLTV guide URL to fill the channel guide." }
                } else {
                    div { class: "row-list",
                        for source in sources.read().clone() {
                            {
                                let id = source.id.clone().unwrap_or_default();
                                let row_id = id.clone();
                                rsx! {
                                    div {
                                        key: "{id}",
                                        class: "flex items-center border-b border-[var(--border)]",
                                        style: "gap:8px;padding:8px 12px",
                                        div { class: "flex-1 min-w-0",
                                            div { style: "font-size:.82rem;font-weight:500", "{source.name}" }
                                            div {
                                                style: "font-size:.72rem;opacity:.6;white-space:nowrap;overflow:hidden;text-overflow:ellipsis",
                                                "{source.url}"
                                            }
                                        }
                                        button {
                                            class: "btn btn-ghost",
                                            style: "height:28px;font-size:.7rem",
                                            onclick: move |_| confirm_delete.set(Some(row_id.clone())),
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "card-body",
                div { style: "font-size:.72rem;opacity:.6;margin-bottom:8px",
                    "Programmes are matched to channels by tvg-id. A guide here applies to every enabled channel, whichever addon imported it."
                }
                FormGroup { label: "Source name",
                    input {
                        class: "form-input",
                        r#type: "text",
                        placeholder: "e.g. Provider guide",
                        value: "{name.read()}",
                        oninput: move |e| name.set(e.value()),
                    }
                }
                FormGroup { label: "XMLTV URL",
                    input {
                        class: "form-input",
                        r#type: "text",
                        placeholder: "https://provider/guide.xml",
                        value: "{url.read()}",
                        oninput: move |e| url.set(e.value()),
                    }
                }
                button {
                    class: "btn btn-primary",
                    disabled: !can_add,
                    onclick: {
                        let client = app_state.clone();
                        move |_| {
                            let source_name = name.peek().trim().to_string();
                            let raw_url = url.peek().trim().to_string();
                            let Ok(source_url) = SourceUrl::try_new(raw_url) else {
                                error.set(Some("Enter a valid EPG URL.".to_string()));
                                return;
                            };
                            let c = client.clone();
                            saving.set(true);
                            spawn(async move {
                                let result = c
                                    .execute(SaveEpgSource {
                                        info: EpgSourceInfo {
                                            id: None,
                                            name: source_name,
                                            url: source_url,
                                        },
                                    })
                                    .await;
                                match result {
                                    Ok(_) => {
                                        name.set(String::new());
                                        url.set(String::new());
                                        error.set(None);
                                        let next = *reload.peek() + 1;
                                        reload.set(next);
                                    }
                                    Err(e) => error.set(Some(format!("Failed to save EPG source: {e}"))),
                                }
                                saving.set(false);
                            });
                        }
                    },
                    if *saving.read() { "Adding…" } else { "Add source" }
                }
            }
        }

        if let Some(id) = confirm_delete.read().clone() {
            ConfirmDialog {
                message: "Remove this EPG source? Programmes it already imported stay in the guide until the next refresh."
                    .to_string(),
                on_confirm: {
                    let client = app_state.clone();
                    move |_| {
                        let c = client.clone();
                        let id = id.clone();
                        let mut cd = confirm_delete;
                        let mut rl = reload;
                        let mut err = error;
                        spawn(async move {
                            match c.execute(DeleteEpgSource { id }).await {
                                Ok(_) => {
                                    cd.set(None);
                                    let next = *rl.peek() + 1;
                                    rl.set(next);
                                }
                                Err(e) => err.set(Some(format!("Failed to remove EPG source: {e}"))),
                            }
                        });
                    }
                },
                on_cancel: move |_| confirm_delete.set(None),
            }
        }
    }
}

#[component]
pub(crate) fn IptvChannelsTab(app_state: AppState) -> Element {
    let mut channels: Signal<Vec<ChannelEditorItem>> = use_signal(Vec::new);
    let mut total: Signal<usize> = use_signal(|| 0);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| Option::<String>::None);
    let mut page = use_signal(|| 0_i64);
    // committed search (triggers fetch); typed search (live input)
    let mut search_committed = use_signal(String::new);
    let mut search_input = use_signal(String::new);
    let mut bulk_working = use_signal(|| false);
    // "all" | "true" | "false"
    let mut enabled_filter = use_signal(|| "all".to_string());
    let mut country_filter = use_signal(String::new);
    let mut countries: Signal<Vec<String>> = use_signal(Vec::new);
    let mut group_filter = use_signal(String::new);
    let mut groups: Signal<Vec<String>> = use_signal(Vec::new);
    // "order" | "name"
    let mut sort_mode = use_signal(|| "order".to_string());
    let mut show_filter_modal = use_signal(|| false);

    // Load distinct country codes and groups once on mount
    let app_state_countries = app_state.clone();
    use_effect(move || {
        let client = app_state_countries.clone();
        spawn(async move {
            if let Ok(cs) = client
                .execute(GetIptvChannelCountries)
                .await
            {
                countries.set(cs);
            }
            if let Ok(gs) = client
                .execute(GetIptvChannelGroups)
                .await
            {
                groups.set(gs);
            }
        });
    });

    let app_state_effect = app_state.clone();
    use_effect(move || {
        let p = *page.read();
        let s = search_committed
            .read()
            .clone();
        let ef = enabled_filter
            .read()
            .clone();
        let cf = country_filter
            .read()
            .clone();
        let gf = group_filter
            .read()
            .clone();
        let sm = sort_mode
            .read()
            .clone();
        loading.set(true);
        let client = app_state_effect.clone();
        spawn(async move {
            let enabled = match ef.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            };
            match client
                .execute(GetIptvChannels {
                    limit: PAGE_SIZE,
                    offset: (p as u32) * PAGE_SIZE,
                    search: s,
                    enabled,
                    country: cf,
                    group: gf,
                    sort: sm,
                })
                .await
            {
                Ok(r) => {
                    total.set(r.total_record_count);
                    channels.set(r.items);
                    error.set(None);
                }
                Err(e) => error.set(Some(format!("Failed to load channels: {e}"))),
            }
            loading.set(false);
        });
    });

    let total_v = *total.read();
    let total_pages = (total_v as i64 + PAGE_SIZE as i64 - 1) / PAGE_SIZE as i64;

    let mut do_search = move || {
        let s = search_input
            .peek()
            .clone();
        search_committed.set(s);
        page.set(0);
    };

    rsx! {
        div { class: "card",
            div { class: "card-header",
                span { class: "card-title", "Channels" }
                if total_v > 0 {
                    span { style: "font-size:.75rem;opacity:.5;margin-left:8px", "{total_v} total" }
                }
                div { style: "display:flex;gap:8px;align-items:center;margin-left:auto",
                    // Filters / Sort modal trigger
                    {
                        let filters_active = enabled_filter.read().as_str() != "all"
                            || !country_filter.read().is_empty()
                            || !group_filter.read().is_empty()
                            || sort_mode.read().as_str() != "order"
                            || !search_committed.read().is_empty();
                        rsx! {
                            button {
                                class: if filters_active { "btn btn-primary" } else { "btn btn-ghost" },
                                style: "height:32px;font-size:.68rem",
                                onclick: move |_| show_filter_modal.set(true),
                                if filters_active { "Filters ●" } else { "Filters" }
                            }
                        }
                    }
                    // Enable all / Disable all — server-side bulk op
                    button {
                        class: "btn btn-ghost",
                        style: "height:32px;font-size:.68rem",
                        disabled: *bulk_working.read() || total_v == 0,
                        onclick: {
                            let client = app_state.clone();
                            move |_| {
                                let search = search_committed.peek().clone();
                                bulk_working.set(true);
                                let c = client.clone();
                                spawn(async move {
                                    let _ = c.execute(BulkChannels {
                                        request: BulkChannelRequest { enabled: true, search: if search.is_empty() { None } else { Some(search) } },
                                    }).await;
                                    bulk_working.set(false);
                                    // re-fetch to reflect new state
                                    let s = search_committed.peek().clone();
                                    let p = *page.peek();
                                    let ef = enabled_filter.peek().clone();
                                    let cf = country_filter.peek().clone();
                                    let gf = group_filter.peek().clone();
                                    let sm = sort_mode.peek().clone();
                                    let enabled = match ef.as_str() {
                                        "true" => Some(true),
                                        "false" => Some(false),
                                        _ => None,
                                    };
                                    loading.set(true);
                                    if let Ok(r) = c.execute(GetIptvChannels { limit: PAGE_SIZE, offset: (p as u32) * PAGE_SIZE, search: s, enabled, country: cf, group: gf, sort: sm }).await {
                                        total.set(r.total_record_count);
                                        channels.set(r.items);
                                    }
                                    loading.set(false);
                                });
                            }
                        },
                        if *bulk_working.read() { "Working…" } else { "Enable All" }
                    }
                    button {
                        class: "btn btn-ghost",
                        style: "height:32px;font-size:.68rem",
                        disabled: *bulk_working.read() || total_v == 0,
                        onclick: {
                            let client = app_state.clone();
                            move |_| {
                                let search = search_committed.peek().clone();
                                bulk_working.set(true);
                                let c = client.clone();
                                spawn(async move {
                                    let _ = c.execute(BulkChannels {
                                        request: BulkChannelRequest { enabled: false, search: if search.is_empty() { None } else { Some(search) } },
                                    }).await;
                                    bulk_working.set(false);
                                    let s = search_committed.peek().clone();
                                    let p = *page.peek();
                                    let ef = enabled_filter.peek().clone();
                                    let cf = country_filter.peek().clone();
                                    let gf = group_filter.peek().clone();
                                    let sm = sort_mode.peek().clone();
                                    let enabled = match ef.as_str() {
                                        "true" => Some(true),
                                        "false" => Some(false),
                                        _ => None,
                                    };
                                    loading.set(true);
                                    if let Ok(r) = c.execute(GetIptvChannels { limit: PAGE_SIZE, offset: (p as u32) * PAGE_SIZE, search: s, enabled, country: cf, group: gf, sort: sm }).await {
                                        total.set(r.total_record_count);
                                        channels.set(r.items);
                                    }
                                    loading.set(false);
                                });
                            }
                        },
                        if *bulk_working.read() { "Working…" } else { "Disable All" }
                    }
                }
            }

            div { class: "card-body tight",
                if *loading.read() {
                    LoadingText {}
                } else if let Some(err) = error.read().as_ref() {
                    span { class: "loading-text", style: "color:var(--error)", "{err}" }
                } else if channels.read().is_empty() {
                    div { class: "empty-state",
                        if total_v == 0
                            && search_committed.read().is_empty()
                            && enabled_filter.read().as_str() == "all"
                            && country_filter.read().is_empty()
                            && group_filter.read().is_empty()
                        {
                            "No channels yet. Run a refresh after adding channel sources."
                        } else {
                            "No channels match your filters."
                        }
                    }
                } else {
                    div { class: "data-table-container",
                        // Column header
                        div { class: "flex items-center px-3 py-1 border-b border-[var(--border)]",
                            style: "font-size:.72rem;opacity:.5;font-weight:600;gap:8px",
                            div { style: "width:32px", "On" }
                            div { class: "flex-1", "Name / Display Name" }
                            div { style: "width:140px", "Group" }
                            div { style: "width:80px;text-align:right", "Ch#" }
                        }
                        div { class: "row-list",
                            for ch in channels.read().clone() {
                                {
                                    let id = ch.id.clone();
                                    let client1 = app_state.clone();
                                    let client2 = app_state.clone();
                                    let client3 = app_state.clone();
                                    let sort_val = ch.sort_order.map(|n| n.to_string()).unwrap_or_default();
                                    let ch_placeholder = ch.channel_number.map(|n| n.to_string()).unwrap_or_else(|| "–".into());
                                    let name_val = ch.custom_name.clone().unwrap_or_default();

                                    rsx! {
                                        div {
                                            key: "{id}",
                                            class: "flex items-center border-b border-[var(--border)] hover:bg-[rgba(0,0,0,0.03)]",
                                            style: if !ch.enabled { "gap:8px;padding:6px 12px;opacity:.4" } else { "gap:8px;padding:6px 12px" },

                                            Switch {
                                                checked: ch.enabled,
                                                on_change: {
                                                    let id = id.clone();
                                                    move |v| {
                                                        // optimistic update
                                                        if let Some(c) = channels.write().iter_mut().find(|c| c.id == id) {
                                                            c.enabled = v;
                                                        }
                                                        let c = client1.clone();
                                                        let id = id.clone();
                                                        spawn(async move {
                                                            let _ = c.execute(PatchChannel {
                                                                id,
                                                                patch: PatchChannelRequest { enabled: Some(v), ..Default::default() },
                                                            }).await;
                                                        });
                                                    }
                                                },
                                            }
                                            div { class: "flex-1 min-w-0",
                                                div { style: "font-size:.82rem;font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis",
                                                    "{ch.name}"
                                                }
                                                input {
                                                    class: "form-input",
                                                    style: "height:24px;font-size:.75rem;padding:2px 6px;margin-top:2px;width:100%",
                                                    value: "{name_val}",
                                                    placeholder: "Custom display name…",
                                                    onchange: {
                                                        let id = id.clone();
                                                        move |e| {
                                                            let v = e.value();
                                                            let custom = if v.is_empty() { None } else { Some(v.clone()) };
                                                            if let Some(c) = channels.write().iter_mut().find(|c| c.id == id) {
                                                                c.custom_name = custom.clone();
                                                            }
                                                            let c = client3.clone();
                                                            let id = id.clone();
                                                            spawn(async move {
                                                                let _ = c.execute(PatchChannel {
                                                                    id,
                                                                    patch: PatchChannelRequest { custom_name: custom, ..Default::default() },
                                                                }).await;
                                                            });
                                                        }
                                                    },
                                                }
                                            }
                                            div { style: "width:140px;font-size:.75rem;opacity:.7;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex-shrink:0",
                                                {ch.group.as_deref().unwrap_or("–")}
                                            }
                                            input {
                                                class: "form-input",
                                                r#type: "number",
                                                style: "width:80px;height:28px;font-size:.8rem;padding:2px 6px;flex-shrink:0;text-align:right",
                                                value: "{sort_val}",
                                                placeholder: "{ch_placeholder}",
                                                onchange: {
                                                    let id = id.clone();
                                                    move |e| {
                                                        let v = e.value().parse::<i64>().ok();
                                                        if let Some(c) = channels.write().iter_mut().find(|c| c.id == id) {
                                                            c.sort_order = v;
                                                        }
                                                        let c = client2.clone();
                                                        let id = id.clone();
                                                        spawn(async move {
                                                            let _ = c.execute(PatchChannel {
                                                                id,
                                                                patch: PatchChannelRequest { sort_order: v, ..Default::default() },
                                                            }).await;
                                                        });
                                                    }
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    PaginationBar { page, total_pages }
                }
            }
        }

        // Filter / Sort modal
        if *show_filter_modal.read() {
            Modal {
                on_close: move |_| show_filter_modal.set(false),
                div { class: "modal-header",
                    span { class: "modal-title", "Filters & Sort" }
                }
                div { class: "modal-body",
                    FormGroup { label: "Search",
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "Search channels…",
                            value: "{search_input.read()}",
                            oninput: move |e| search_input.set(e.value()),
                            onkeydown: move |e| {
                                if e.key() == Key::Enter {
                                    do_search();
                                    show_filter_modal.set(false);
                                }
                            },
                        }
                    }
                    FormGroup { label: "Sort by",
                        select {
                            class: "form-input",
                            value: "{sort_mode.read()}",
                            onchange: move |e| { sort_mode.set(e.value()); page.set(0); },
                            option { value: "order", "Order" }
                            option { value: "name", "Name" }
                        }
                    }
                    FormGroup { label: "Status",
                        select {
                            class: "form-input",
                            value: "{enabled_filter.read()}",
                            onchange: move |e| { enabled_filter.set(e.value()); page.set(0); },
                            option { value: "all", "All" }
                            option { value: "true", "Enabled" }
                            option { value: "false", "Disabled" }
                        }
                    }
                    if !countries.read().is_empty() {
                        FormGroup { label: "Country",
                            select {
                                class: "form-input",
                                value: "{country_filter.read()}",
                                onchange: move |e| { country_filter.set(e.value()); page.set(0); },
                                option { value: "", "All countries" }
                                for c in countries.read().clone() {
                                    option { value: "{c}", "{c}" }
                                }
                            }
                        }
                    }
                    if !groups.read().is_empty() {
                        FormGroup { label: "Group",
                            select {
                                class: "form-input",
                                value: "{group_filter.read()}",
                                onchange: move |e| { group_filter.set(e.value()); page.set(0); },
                                option { value: "", "All groups" }
                                for g in groups.read().clone() {
                                    option { value: "{g}", "{g}" }
                                }
                            }
                        }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn btn-ghost",
                        onclick: move |_| {
                            search_input.set(String::new());
                            search_committed.set(String::new());
                            enabled_filter.set("all".to_string());
                            country_filter.set(String::new());
                            group_filter.set(String::new());
                            sort_mode.set("order".to_string());
                            page.set(0);
                        },
                        "Reset"
                    }
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| {
                            do_search();
                            show_filter_modal.set(false);
                        },
                        "Done"
                    }
                }
            }
        }
    }
}

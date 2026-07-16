use gtk4::gdk::Display;
use gtk4::CssProvider;

pub fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_data(
        "
        .thumb-button {
            border-radius: 12px;
            border: 2px solid transparent;
            padding: 0;
            background-color: transparent;
            transition: 
                transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1),
                box-shadow 180ms cubic-bezier(0.2, 0.8, 0.2, 1),
                border-color 150ms ease;
        }
        .thumb-button:hover {
            transform: scale(1.03);
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        }
        .thumb-button.selected {
            border-color: #ffb688;
            transform: scale(1.05);
            box-shadow: 0 6px 16px rgba(255, 182, 136, 0.25);
        }
        @keyframes pulse {
            0% { background-color: rgba(255, 255, 255, 0.03); }
            50% { background-color: rgba(255, 255, 255, 0.09); }
            100% { background-color: rgba(255, 255, 255, 0.03); }
        }
        .thumb-button.thumb-loading {
            animation: pulse 1.5s infinite ease-in-out;
        }
        .thumb-button.favorite-flash {
            border-color: #ffb688;
            background-color: rgba(255, 182, 136, 0.3);
            transform: scale(0.97);
        }
        .favorite-heart {
            font-size: 34px;
            opacity: 0;
            color: #ffb688;
            text-shadow: 0 2px 6px rgba(0, 0, 0, 0.5);
        }
        @keyframes heart-pop {
            0% { opacity: 0; transform: scale(0.3); }
            35% { opacity: 1; transform: scale(1.3); }
            60% { transform: scale(0.95); }
            100% { opacity: 0; transform: scale(1.1); }
        }
        @keyframes heart-fade {
            0% { opacity: 1; transform: scale(1); color: #ffb688; }
            100% { opacity: 0; transform: scale(0.5); color: #888888; }
        }
        .favorite-heart-add {
            animation: heart-pop 300ms ease-out;
        }
        .favorite-heart-remove {
            animation: heart-fade 300ms ease-in;
        }
        .page-dot {
            font-size: 10px;
            opacity: 0.4;
            transition: opacity 200ms ease, transform 200ms ease;
        }
        .view-tab {
            font-size: 12px;
            opacity: 0.5;
            padding: 4px 10px;
            border-radius: 8px;
            transition: 
                opacity 150ms ease, 
                background-color 150ms ease, 
                color 150ms ease,
                transform 150ms ease;
        }
        .view-tab:hover {
            opacity: 0.8;
            background-color: rgba(255, 255, 255, 0.05);
            transform: translateY(-1px);
        }
        .view-tab-active {
            opacity: 1.0;
            font-weight: bold;
            color: #ffb688;
            background-color: rgba(255, 182, 136, 0.08);
        }
        .view-tab-active:hover {
            background-color: rgba(255, 182, 136, 0.12);
        }
        ",
    );
    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    } else {
        eprintln!("Warning: Could not style application. No active GDK Display found.");
    }
}

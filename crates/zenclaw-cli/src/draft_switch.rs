    let theme = ColorfulTheme::default();
    
    // Create a flat model list with provider prefixes
    let mut flat_models = Vec::new();
    for p in PROVIDERS {
        for m in p.models {
            flat_models.push((p.name, p.display, *m, p));
        }
    }
    
    // Also let's push a cancel option
    // It's a bit rigid to use a flat list of ALL. Actually, dialoguer FuzzySelect is awesome.
    // Dialoguer's Select has `interact_opt` to escape. 

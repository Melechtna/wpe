const { invoke } = window.__TAURI__.core;

let monitors = [];
let wallpapers = [];
let config = [];
let currentTab = 0;
let isRunning = false;
let hostPort = null;
let isStarting = false;

function getBaseUrl(path) {
    if (!hostPort) return `file://${path}`;
    const encoded = encodeURIComponent(path).replace(/%2F/g, '/');
    return `http://127.0.0.1:${hostPort}/${encoded}`;
}

async function init() {
    try {
        await invoke('init_samples');
        hostPort = await invoke('start_host');
        console.log('Host started on port:', hostPort);
        
        setInterval(async () => {
            if (hostPort) {
                try {
                    await fetch(`http://127.0.0.1:${hostPort}/heartbeat`);
                } catch (e) {
                    console.warn('Host heartbeat failed:', e);
                }
            }
        }, 500);

        setInterval(async () => {
            const newWallpapers = await invoke('get_wallpapers');
            const oldPaths = wallpapers.map(w => w.path).sort();
            const newPaths = newWallpapers.map(w => w.path).sort();
            if (JSON.stringify(oldPaths) !== JSON.stringify(newPaths)) {
                wallpapers = newWallpapers;
                render();
            }
        }, 1000);
    } catch (e) {
        console.error('Failed to start host:', e);
    }
    await loadData();
    render();
    updateButton();
}

async function loadData() {
    try {
        monitors = await invoke('get_monitors');
        config = await invoke('load_config');
        wallpapers = await invoke('get_wallpapers');
        isRunning = await invoke('is_helper_running');

        for (const m of monitors) {
            if (!config.some(c => c.name === m.name)) {
                config.push({ name: m.name, enabled: false, wallpaper: null });
            }
        }
    } catch (e) {
        console.error('Failed to load data:', e);
    }
}

async function saveConfig() {
    try {
        await invoke('save_config', { monitors: config });
    } catch (e) {
        console.error('Failed to save config:', e);
    }
}

function render() {
    renderTabs();
    renderPanels();
}

function renderTabs() {
    const tabsEl = document.getElementById('monitor-tabs');
    tabsEl.innerHTML = config.map((m, i) => `
        <button class="tab ${i === currentTab ? 'active' : ''}" data-index="${i}">
            <span class="fa-solid fa-desktop"></span> ${m.name}
        </button>
    `).join('');

    tabsEl.querySelectorAll('.tab').forEach(tab => {
        tab.addEventListener('click', () => {
            currentTab = parseInt(tab.dataset.index);
            render();
        });
    });
}

function renderPanels() {
    const contentEl = document.getElementById('monitor-content');
    contentEl.innerHTML = config.map((m, i) => {
        const monitor = monitors.find(mon => mon.name === m.name) || { 
            name: m.name, 
            resolution: 'Unknown', 
            refresh_rate: 'Unknown' 
        };
        
        return `
            <div class="monitor-panel ${i === currentTab ? 'active' : ''}" data-index="${i}">
                <div class="monitor-info">
                    <h2><span class="fa-solid fa-desktop"></span> ${monitor.name}</h2>
                    <p>${monitor.resolution} @ ${monitor.refresh_rate}</p>
                </div>
                
                <div class="enabled-row">
                    <input type="checkbox" id="enabled-${i}" ${m.enabled ? 'checked' : ''}>
                    <label for="enabled-${i}">Enabled</label>
                </div>
                
                <div class="wallpaper-header">
                    <h3><span class="fa-solid fa-images"></span> Wallpapers:</h3>
                    <button class="folder-btn"><span class="fa-solid fa-folder-open"></span></button>
                </div>
                
                <div class="wallpaper-grid">
                    ${wallpapers.map(w => {
                        const previewUrl = w.preview_path ? getBaseUrl(w.preview_path) : null;
                        const iframeUrl = getBaseUrl(w.path) + '/index.html';
                        return `
                            <div class="wallpaper-card ${m.wallpaper === w.path ? 'selected' : ''}" data-path="${w.path}" data-index="${i}">
                                <div class="preview-container">
                                    ${previewUrl 
                                        ? `<img class="preview-image" src="${previewUrl}" alt="${w.name}" onerror="this.style.display='none'">`
                                        : `<div class="preview-placeholder">${w.name.substring(0, 2).toUpperCase()}</div>`
                                    }
                                    <iframe class="preview-iframe" data-src="${iframeUrl}"></iframe>
                                </div>
                                <div class="wallpaper-name">
                                    <span>${w.name}</span>
                                    <input type="checkbox" ${m.wallpaper === w.path ? 'checked' : ''}>
                                </div>
                            </div>
                        `;
                    }).join('')}
                </div>
            </div>
        `;
    }).join('');

    contentEl.querySelectorAll('.monitor-panel').forEach(panel => {
        const idx = parseInt(panel.dataset.index);
        
        panel.querySelector(`#enabled-${idx}`).addEventListener('change', (e) => {
            config[idx].enabled = e.target.checked;
            saveConfig();
            updateButton();
        });

        panel.querySelector('.folder-btn').addEventListener('click', async () => {
            await invoke('open_wallpaper_folder');
            await loadData();
            renderPanels();
        });

        const cards = panel.querySelectorAll('.wallpaper-card');
        console.log('Attaching listeners to', cards.length, 'cards for panel', idx);
        
        cards.forEach(card => {
            const path = card.dataset.path;
            const iframe = card.querySelector('.preview-iframe');
            console.log('Card:', path, 'iframe:', iframe, 'data-src:', iframe?.dataset?.src);
            
            card.addEventListener('mouseenter', () => {
                console.log('mouseenter', path, iframe, iframe?.dataset?.src);
                if (iframe && iframe.dataset.src) {
                    iframe.src = iframe.dataset.src;
                }
            });
            
            card.addEventListener('mouseleave', () => {
                if (iframe) {
                    iframe.src = 'about:blank';
                }
            });
            
            card.addEventListener('click', () => {
                config[idx].wallpaper = path;
                saveConfig();
                render();
            });
        });
    });
}

function updateButton() {
    const btn = document.getElementById('start-stop-btn');
    const statusEl = document.getElementById('status-message');
    const canStart = config.some(m => m.enabled && m.wallpaper);

    if (isStarting) {
        btn.innerHTML = '<span class="spinner"></span> Starting...';
        btn.className = 'btn loading';
        btn.disabled = true;
    } else if (isRunning) {
        btn.innerHTML = '<span class="fa-solid fa-stop"></span> Stop';
        btn.className = 'btn stop';
        btn.disabled = false;
        btn.onclick = stopWallpapers;
    } else {
        btn.innerHTML = '<span class="fa-solid fa-play"></span> Start';
        btn.className = 'btn start';
        btn.disabled = !canStart;
        btn.onclick = startWallpapers;
    }

    if (!canStart && !isRunning) {
        statusEl.textContent = 'Enable a monitor and select a wallpaper to start';
    } else {
        statusEl.textContent = '';
    }
}

async function startWallpapers() {
    console.log('startWallpapers called');
    await saveConfig();
    await new Promise(r => setTimeout(r, 100));
    if (!hostPort) {
        console.error('Host not running');
        return;
    }
    isStarting = true;
    updateButton();
    try {
        console.log('Calling start_wallpapers with port', hostPort);
        await invoke('start_wallpapers', { port: hostPort });
        isRunning = true;
    } catch (e) {
        console.error('Failed to start wallpapers:', e);
    }
    isStarting = false;
    updateButton();
}

async function stopWallpapers() {
    console.log('stopWallpapers called');
    try {
        await invoke('stop_wallpapers');
        isRunning = false;
        updateButton();
    } catch (e) {
        console.error('Failed to stop wallpapers:', e);
    }
}

document.addEventListener('contextmenu', e => e.preventDefault());
document.addEventListener('keydown', e => {
    if (e.key === 'F5') {
        e.preventDefault();
    }
});

document.addEventListener('DOMContentLoaded', init);

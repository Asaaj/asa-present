function format(code, newline_at_end = true) {
    let lines = code.split(/\r?\n|\r|\n/g);

    const indents = lines
        .map((line) => line.trimEnd().length > 0 ? line.length - line.trimStart().length : null)
        .filter((len) => len !== null);
    const shortest_indent = Math.min(...indents);

    lines = lines.map((line) => line.slice(shortest_indent));
    code = lines.join("\n").trim();
    if (newline_at_end) {
        code += "\n";
    }
    return code;
}

function make_editor(element) {
    const starter_code = format(element.textContent);
    element.innerHTML = "";
    window[element.id] = monaco.editor.create(element, {
        value: starter_code,
        language: 'rust',
        scrollBeyondLastLine: false,
        minimap: {enabled: false},
        overviewRulerLanes: 0,
        hideCursorInOverviewRuler: true,
        scrollbar: {
            vertical: 'hidden'
        },
        overviewRulerBorder: false,
    });
}

async function compile(button) {
    button.disabled = true;
    try {
        const code = window.demo_code.getValue();
        resp = await fetch("http://127.0.0.1:8000/compile", {
            method: "POST",
            body: JSON.stringify({
                source_code: code,
                language: "rust"
            })
        });
        console.log(await resp.json());
    } finally {
        button.disabled = false;
    }
}

function insert_all_editors(class_name) {
    document.querySelectorAll(class_name).forEach((element) => {
        make_editor(element);
    })
}

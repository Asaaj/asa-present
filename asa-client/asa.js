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

let DOWNLOAD_INDEX = 0;

async function compile(button, editor_id) {
    button.disabled = true;
    try {
        const code = window[editor_id].getValue();
        const response = await fetch("http://127.0.0.1:8000/compile", {
            cache: "no-store",
            method: "POST",
            body: JSON.stringify({
                source_code: code,
                package_name: editor_id,
                language: "rust",
            })
        });

        const response_json = await response.json();
        console.log(response_json);
        if (response_json["exit_detail"]) {
            console.log("Error!");
            console.log("stdout = ", response_json["stdout"]);
            console.log("stderr = ", response_json["stderr"]);
        } else {
            // Use a unique URL so module caching isn't a problem.
            // Note: apparently leaves the old module loaded. Memory leak
            const download_url = "/" + response_json.result + "?num=" + DOWNLOAD_INDEX;
            let module = await import(download_url);
            DOWNLOAD_INDEX += 1;
            await module.default();
            return module;
        }
    } finally {
        button.disabled = false;
    }
    return null;
}

async function compile_and_run(button, editor_id) {
    const wasm = await compile(button, editor_id);

    const run_editor_id = editor_id + "_run";
    const code = window[run_editor_id].getValue();
    console.log(code)
    eval(code)(wasm)
}

function insert_all_editors(class_name) {
    document.querySelectorAll(class_name).forEach((element) => {
        make_editor(element);
    })
}

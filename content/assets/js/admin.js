(function (D) {
    const saveForm = D.querySelector('#save-article');
    const articleList = D.querySelector('#admin-article-manager ol');
    const createNew = D.querySelector('#admin-new-article');
    const editorSection = D.querySelector('#admin-article-editor');
    const editor = D.querySelector('#article-editor-input');
    const saveBtn = D.querySelector('#save-article button');
    const successMsg = D.querySelector('#save-success');
    const listSectionTabContaner = D.querySelector('#admin-list-sections-tabs');
    const listSectionTabs = Array.from(D.querySelectorAll('#admin-list-sections-tabs li'));
    const listSections = Array.from(D.querySelectorAll('.tab-content[data-tab-set="admin-list-section"]'));

    const imageList = D.querySelector('.image-list');

    const ALT_PLACEHOLDER = 'image caption';

    function fetchArticleText(slug) {
        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            editor.value = xhr.responseText;
            editorSection.classList.remove('new');
            saveForm.dataset.slug = slug;
            saveForm.dataset.method = 'PUT';
            window.location.hash = slug;
            editor.disabled = false;
        });
        xhr.open('GET', `/article/${slug}/text`);
        xhr.send();
    }

    function confirmDelete(slug, title) {
        if (!window.confirm(
            `"${title}"\n\nAre you sure you want to delete this article?\n\nWARNING: this cannot be undone!`
        )) return;

        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            const li = articleList.querySelector(`li[data-slug="${slug}"]`);
            if (!li) return;
            li.parentNode.removeChild(li);
            editor.value = '';
        });
        xhr.open('DELETE', `/article/${slug}`);
        xhr.send();
    }

    articleList.addEventListener('click', event => {
        const target = event.target;
        if (target.nodeName === 'A') {
            if (!target.dataset.slug) return;
            fetchArticleText(target.dataset.slug);
        } else if (target.nodeName === 'BUTTON') {
            confirmDelete(
                target.dataset.slug,
                target.dataset.title
            );
        }
    });

    createNew.addEventListener('submit', event => {
        event.preventDefault();
        editor.value = '';
        editorSection.classList.add('new');
        saveForm.dataset.method = 'POST';
        delete saveForm.dataset.slug;
        window.location.hash = '';
        editor.disabled = false;
        editor.focus();
    })

    saveForm.addEventListener('submit', event => {
        event.preventDefault();

        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            saveBtn.disabled = false;
            saveBtn.textContent = 'Save';
            setTimeout(() => { successMsg.classList.remove('visible') }, 1000);
            if (editorSection.classList.contains('new')) {
                editorSection.classList.remove('new');
                const frag = D.createElement('template');
                frag.innerHTML = xhr.responseText;
                articleList.prepend(frag.content);
                setTimeout(() => {
                    const slug = articleList.querySelector('li:first-child').dataset.slug;
                    fetchArticleText(slug);
                }, 0);
            }
        });

        const path = saveForm.dataset.method === 'POST' ?
            '/articles' :
            `/article/${saveForm.dataset.slug}`;
        xhr.open(saveForm.dataset.method, path);
        xhr.setRequestHeader("Content-Type", "application/json");
        xhr.send(editor.value);

        saveBtn.disabled = true;
        saveBtn.textContent = '...';
        successMsg.classList.add('visible');
    });

    if (window.location.hash) {
        const slug = window.location.hash.replace(/^#/, '');
        fetchArticleText(slug);
    }

    function activateTabContent(sectionId) {
        listSections.forEach(s => s.classList.remove('active'));
        D.querySelector(sectionId).classList.add('active');

    }

    listSectionTabContaner.addEventListener('click', event => {
        if (event.target.nodeName !== 'A') return;
        event.preventDefault();

        activateTabContent(event.target.getAttribute('href'));
        listSectionTabs.forEach(t => t.classList.remove('active'));
        event.target.parentNode.classList.add('active');
    });

    function loadImageList() {
        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            if (xhr.status === 200) {
                imageList.innerHTML = xhr.response;
            } else {
                imageList.innerHTML = "Failed to fetch image list :("
            }
        });

        xhr.open('GET', imageList.dataset.source);
        xhr.send();
    }

    function insertImageRef(path) {
        if (editor.disabled) return;
        let insertAt = editor.selectionStart;
        const beforeText = editor.value.substring(0, insertAt);
        const afterText = editor.value.substring(insertAt, editor.value.length);
        let beforeSpacer, afterSpacer;
        if (/\n\n$/.test(beforeText)) {
            beforeSpacer = '';
        } else if (/\n$/.test(beforeText)) {
            beforeSpacer = '\n';
        } else {
            beforeSpacer = '\n\n';
        }
        if (/^\n\n/.test(afterText)) {
            afterSpacer = '';
        } else if (/^\n/.test(afterText)) {
            afterSpacer = '\n';
        } else {
            afterSpacer = '\n\n';
        }
        editor.value = `${beforeText}${beforeSpacer}![${ALT_PLACEHOLDER}](${path})${afterSpacer}${afterText}`;
        insertAt += 2 + beforeSpacer.length;
        editor.setSelectionRange(insertAt, insertAt + ALT_PLACEHOLDER.length, 'forward');
        editor.focus();
    }

    function getAncestor(node, ancestorNodeName) {
        if (node.nodeName === ancestorNodeName) return node;
        if (node.nodeName === 'BODY') return false;
        return getAncestor(node.parentNode, ancestorNodeName);
    }

    function deleteImage(path) {
        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            if (xhr.status === 200) {
                imageList.innerHTML = xhr.response;
            } else {
                alert(`Failed to delete image ${path}`);
            }
        });

        xhr.open('DELETE', path);
        xhr.send();
    }

    function handleThumbClick(el, shift) {
        if (shift) {
            window.open(el.dataset.path, el.fileName);
        } else {
            insertImageRef(el.dataset.path);
        }
    }

    imageList.addEventListener('click', event => {
        const el = event.target;
        switch (el.nodeName) {
            case 'H4': {
                el.classList.toggle('collapsed');
                break;
            }
            case 'BUTTON': {
                const thumb = getAncestor(el, 'FIGURE');
                if (thumb && confirm(`Are you sure you want to delete the image ${thumb.dataset.fileName}?`)) {
                    deleteImage(thumb.dataset.path);
                }
                break;
            }
            default: {
                const thumb = getAncestor(el, 'FIGURE');
                if (thumb) {
                    handleThumbClick(thumb, event.shiftKey);
                }
                break;
            }
        }
    });

    activateTabContent(D.querySelector('.tab.active a').getAttribute('href'));

    loadImageList();
}(window.document));


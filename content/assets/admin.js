(function (D) {
    const saveForm = D.querySelector('#save-article');
    const articleList = D.querySelector('#admin-article-manager ol');
    const createNew = D.querySelector('#admin-new-article');
    const editorSection = D.querySelector('#admin-article-editor');
    const editor = D.querySelector('.article-editor');
    const saveBtn = D.querySelector('#save-article button');
    const successMsg = D.querySelector('#save-success');

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
        xhr.open('GET', `/articles/${slug}/text`);
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
        xhr.open('DELETE', `/articles/${slug}`);
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
            `/articles/${saveForm.dataset.slug}`;
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
}(window.document));


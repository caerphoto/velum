(function (D) {
    const form = D.querySelector('#save-article');
    const articleList = D.querySelector('#admin-article-manager ol');
    const editor = D.querySelector('.article-editor');
    const saveBtn = D.querySelector('#save-article button');

    let slug = '';

    function fetchArticleText() {
        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            editor.value = xhr.responseText;
        });
        xhr.open('GET', `/articles/${slug}/text`);
        xhr.send();
    }

    articleList.addEventListener('click', event => {
        const target = event.target;
        if (target.nodeName !== 'A') return;
        slug = target.getAttribute('data-slug');
        fetchArticleText();
    });

    form.addEventListener('submit', event => {
        event.preventDefault();
        const articleText = editor.value;
        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            saveBtn.disabled = false;
        });
        const path = `${form.getAttribute('data-action')}/${slug}`;
        xhr.open('PUT', path);
        xhr.setRequestHeader("Content-Type", "application/json");
        xhr.send(articleText);
        saveBtn.disabled = true;
    });

    if (window.location.hash) {
        slug = window.location.hash.replace(/^#/, '');
        fetchArticleText();
    }
}(window.document));


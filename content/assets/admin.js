(function (D) {
    const saveForm = D.querySelector('#save-article');
    const articleList = D.querySelector('#admin-article-manager ol');
    const editor = D.querySelector('.article-editor');
    const saveBtn = D.querySelector('#save-article button');
    const successMsg = D.querySelector('#save-success');

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
        if (!slug) return;
        fetchArticleText();
    });

    saveForm.addEventListener('submit', event => {
        event.preventDefault();
        const articleText = editor.value;
        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            saveBtn.disabled = false;
            saveBtn.textContent = 'Save';
            setTimeout(() => { successMsg.classList.remove('visible') }, 1000);
        });
        const path = `${saveForm.getAttribute('data-action')}/${slug}`;
        xhr.open('PUT', path);
        xhr.setRequestHeader("Content-Type", "application/json");
        xhr.send(articleText);

        saveBtn.disabled = true;
        saveBtn.textContent = '...';
        successMsg.classList.add('visible');
    });

    if (window.location.hash) {
        slug = window.location.hash.replace(/^#/, '');
        fetchArticleText();
    }
}(window.document));


(function (D) {
    const commentList = D.querySelector('#comments-list');
    const form = D.querySelector('#comment-form');

    function appendComment(html) {
        const tpl = D.createElement('template');
        tpl.innerHTML = html;
        const li = tpl.content.querySelector('li');
        if (li) { li.classList.add('new'); }
        commentList.appendChild(tpl.content);

        form.reset();
    }

    form.addEventListener('submit', event => {
        event.preventDefault();

        let formData = new URLSearchParams(new FormData(form));

        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            appendComment(xhr.responseText);
        });
        xhr.open('POST', form.action);
        xhr.setRequestHeader("Content-Type", "application/x-www-form-urlencoded");
        xhr.send(formData.toString());
    });
}(window.document));

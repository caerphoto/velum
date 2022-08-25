(function (D) {
    const commentList = D.querySelector('#comments-list');
    const form = D.querySelector('#comment-form');

    function commentPosted(html) {
        const tpl = D.createElement('template');
        tpl.innerHTML = html
        commentList.appendChild(tpl.content);

        form.reset();
    }

    form.addEventListener('submit', event => {
        event.preventDefault();

        let formData = new URLSearchParams(new FormData(form));

        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', () => {
            commentPosted(xhr.responseText);
        });
        xhr.open('POST', form.action);
        xhr.setRequestHeader("Content-Type", "application/x-www-form-urlencoded");
        xhr.send(formData.toString());
    });
}(window.document));

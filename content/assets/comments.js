(function (D) {
    const commentList = D.querySelector('#comments-list');
    const form = D.querySelector('#comment-form');

    function commentPosted(comment) {
        console.log(comment);
    }

    form.addEventListener('submit', event => {
        event.preventDefault();

        let formData = new FormData();
        ['author', 'author_url', 'text'].forEach(field => {
            const val = form.elements[field].value;
            formData.append(field, val);
        });

        const xhr = new XMLHttpRequest();
        xhr.addEventListener('load', commentPosted);
        xhr.open('POST', form.action);
        xhr.send(formData);
    });
}(window.document));

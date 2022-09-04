(function (D) {
    function setupComments() {
        const commentList = D.querySelector('#comments-list');
        const form = D.querySelector('#comment-form');
        const submitBtn = form.querySelector('[type="submit"]');

        function appendComment(html) {
            const tpl = D.createElement('template');
            tpl.innerHTML = html;
            const li = tpl.content.querySelector('li');
            if (li) { li.classList.add('new'); }
            commentList.appendChild(tpl.content);
            commentList.querySelector('li:last-child').scrollIntoView({ behavior: 'smooth' });
        }

        form.addEventListener('submit', event => {
            event.preventDefault();

            const formData = new URLSearchParams(new FormData(form));

            const xhr = new XMLHttpRequest();
            xhr.addEventListener('load', () => {
                appendComment(xhr.responseText);
                form.reset();
                submitBtn.disabled = false;
            });
            xhr.open('POST', form.getAttribute('data-action'));
            xhr.setRequestHeader("Content-Type", "application/x-www-form-urlencoded");
            xhr.send(formData.toString());
            submitBtn.disabled = true;
        });
    }

    setTimeout(setupComments, 0);
}(window.document));

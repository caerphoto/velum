(function () {
  var pop;

  function positionFromImage(img) {
    var clientRect = img.getBoundingClientRect();
    var popoverRect = pop.getBoundingClientRect();

    return {
      left: clientRect.left + window.scrollX,
      top: clientRect.top + clientRect.height - popoverRect.height +
        window.scrollY
    };
  }

  function showPopoverOnImage(event) {
    var popoverPosition;
    var img = event.target;

    if (img.nodeName !== 'IMG') return;
    if (img.title === '') return;

    popoverPosition = positionFromImage(img);
    pop.style.display = 'fixed';
    pop.style.top = popoverPosition.top + 'px';
    pop.style.left = popoverPosition.left + 'px';

    pop.textContent = img.title;
  }

  function hidePopover() {
    pop.classList.remove('active');
  }

  pop = document.createElement('div');
  pop.id = 'img-title-popover';
  pop.style.display = 'none';
  document.body.appendChild(pop);
  document.body.addEventListener('mouseover', showPopoverOnImage);
  document.body.addEventListener('mouseout', hidePopover);
}());

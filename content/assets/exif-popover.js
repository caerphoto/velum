(function () {
  var exifPopover;

  function positionFromImage(img) {
    var clientRect = img.getBoundingClientRect();
    var popoverRect = exifPopover.getBoundingClientRect();

    return {
      left: clientRect.left + window.scrollX,
      top: clientRect.top + clientRect.height - popoverRect.height +
        window.scrollY
    };
  }

  function showExifOnImage(event) {
    var popoverPosition;
    var img = event.target;

    if (img.nodeName !== 'IMG') {
      return;
    }
    exifPopover.classList.add('active');
    exifPopover.classList.add('loading');
    popoverPosition = positionFromImage(img);

    exifPopover.style.top = popoverPosition.top + 'px';
    exifPopover.style.left = popoverPosition.left + 'px';

    window.EXIF.getData(img, function () {
      var data = this.exifdata;
      if (Object.keys(data).length === 0) {
        return;
      }
      var model = data.LensModel || '(lens name unavailable)';
      var length = Math.round(data.FocalLength) || '--';
      var aperture = data.FNumber || '--';

      model = model.replace('Fujifilm Fujinon', '');

      exifPopover.innerHTML = [
        length + 'mm',
        'f/' + aperture
      ].join(' &middot; ') +
        '<br><span class="lens-name">' + model + '</span>';

      exifPopover.classList.remove('loading');
    });
  }

  function hideExif() {
    exifPopover.classList.remove('active');
  }

  if (window.EXIF) {
    exifPopover = document.createElement('div');
    exifPopover.id = 'exif-popover';
    document.body.appendChild(exifPopover);
    document.body.addEventListener('mouseover', showExifOnImage);
    document.body.addEventListener('mouseout', hideExif);
  }
}());

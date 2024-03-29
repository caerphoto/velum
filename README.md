# Velum

A very simple blog engine: it reads your page templates and Markdown posts and
combines them into a full blog site, which it serves either directly or via
something like nginx. The idea is to be as minimal and lightweight as possible,
while still providing a decent blog experience for your readers.

Live example: <https://blog.andyf.me/> running behind nginx.

## In-memory Storage

For performance reasons, rather than load the article file from the filesystem
each time it's needed, the articles are read once on startup from
`content/articles` and stored in memory. The title and tags are also extracted
from each article, using the assumption that the first line is of the format `#
Article Title`, and tags are on the second line, in the form `|these, are,
some, tags|`. A 'slug', i.e. a simplified version of the title, is also
generated, for use in URL routing, along with a timestamp, and all of these are
stored with the title-less content.

## Images

The admin page lists all images currently available, any of which can be clicked
to insert the appropriate Markdown code into the current article in the editor.

Images can be uploaded or deleted via the admin page, with a maximum upload size
of 25MB total. Uploaded images will be stored in the configured content
directory under `images/<4-digit-year>/<2-digit-month>`, to help with
organisation.

When images are uploaded, thumbnail versions of them will be generated
automatically, to avoid the need to load the full-sized versions in the admin
page image list. This can be quite a lengthy process if large and/or many images
are uploaded, but a progress bar will be displayed that shows how many
thumbnails are left to generate. Thumbnails are generally around 3–7KB.

If images were uploaded via other means, e.g. by FTP, thumbnail generation will
begin automatically when the admin page is loaded. As with uploading via the
admin page, a progress bar will be displayed.

## Comments

There is a fairly basic commenting system in place that simply stores comment
author name, their URL (optional), and a plain-text comment limited to 3000
characters (not configurable yet).

While comments are also stored in memory while the server is running, they are
also backed up to a file (`content/comments.jsonl` by default) so they can be
restored if the server needs to be restarted. The format, JSONL, is
a line-based variant of JSON – each line is its own independent JSON object
representing a single comment.

There is currently no facility for managing comments, but this is something
I plan to implement, hence the tab on the admin page.

Comments are rate-limited by IP address, to prevent some potential abuse. The
limit is 2 seconds, and not currently configurable. There are some other
options I want to explore in this area, including possible cookie-based
'authentication', and more.

Comments are also write-only, and I'm not sure whether this is something I want
to expand on. Making comments editable means implementing a whole system of
user accounts, login, etc., and I'm just not sure it's worth the extra
complication for what's supposed to be a lightweight blog engine.

## RSS Feed

There's a link in the page footer for an RSS feed, that lists the most recent 10
articles. Images and links in articles that use relative URLs, e.g.
"/content/images/example.jpg" will be rewritten dynamically to include the full
blog URL, e.g. "https://blog.andyf.me/content/images/example.jpg", so that
images display properly and links open properly in RSS feed readers.

## Getting started

Assuming you have a functional Rust environment, you can compile and run the
server with `cargo run`. On startup the articles files are read, as described
above, and the Handlebars templates are read from `content/templates`. If
running in development mode, the templates are not cached, and are re-read each
time they're needed. At the moment this setting is controlled directly in the
source code, but in future it will be controllable using a command-line
argument (see To Do, below).

Once the server has started, visit <http://localhost:3090/> in your browser to
see the index page, showing a list of articles. The default is 10 per page,
adjustable via `Settings.toml` along with a couple of other things.

## Running in production

Production use is basically the same as development use, at present. In the
future there may be options for running daemonised, or as a system service.

## To Do

2. Ensure there are no duplicate slugs.

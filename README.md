# Velum

A very simple blog engine: it reads your page templates and Markdown posts and
combines them into a full blog site, which it serves either directly or via
something like nginx. The idea is to be as minimal and lightweight as possible,
while still providing a decent blog experience for your readers.

## In-memory Storage

For performance reasons, rather than load the article file from the filesystem
each time it's needed, the articles are read once on startup from
`content/articles` and stored in memory. The title and tags are also extracted
from each article, using the assumption that the first line is of the format `#
Article Title`, and tags are on the second line, in the form `|these, are,
some, tags|`. A 'slug', i.e. a simplified version of the title, is also
generated, for use in URL routing, along with a timestamp, and all of these are
stored with the title-less content.

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

1. Since removing the reliance on Redis, it's become clear that there's no
   proper handling of duplicate slugs/titles. While this shouldn't really be
   a problem, and nothing will *break*, exactly, it's obviously not ideal, and
   a solution needs to be found.

2. Comments: not every blog wants or needs them, but they should be included,
   as the third-party options like Disqus, while easy to add, do not really
   integrate well into the styling of the page, being iframes and all.

3. Rebuilding the article database: adding a new article currently means
   restarting the server is necessary in order for it to be included in the
   article list, but  this obviously means downtime, and is not great from a UX
   perspective. A better option would be a way to send a special HTTP request
   to the server that signals it to rebuild, possibly via an admin page of some
   kind. There is already code in place to rebuild the article cache, so it's
   mostly a matter of wiring it up to a UI.

4. Finally there's the question of an editor: does Velum even need one? If not,
   what about a simplified way to upload content, that means users don't have
   to manually copy files (including images) to their server? As with the
   server restart issue, this is about UX for the blog maintainer, obviating
   the need for SSHing into the server and other such disagreeableness.

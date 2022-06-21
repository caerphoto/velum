# Velum

A very simple blog engine: it reads your page templates and Markdown posts and combines them into a full blog site, which it serves either directly or via something like nginx.

**Requires Redis for article storage.**

On startup, it reads all the `.md` files in `content/articles`, extracts a title, timestamp and 'slug' (simplified title used as part of the article's URL), and stores these in Redis. When an article is requested, the Markdown is read from Redis and insterted into a Handlebars-based template, which is then rendered and served in response.

The blog has pagination and 'next/previous' links within articles, and tags are on the to-do list.

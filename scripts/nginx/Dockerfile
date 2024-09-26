FROM nginx:mainline-bookworm

RUN openssl dhparam -out /etc/nginx/dhparam.pem 2048

COPY nginx.conf /etc/nginx/nginx.conf
COPY auohp.dev.crt auohp.dev.key /etc/nginx/tls/

CMD ["nginx", "-g", "daemon off;", "-c", "/etc/nginx/nginx.conf"]

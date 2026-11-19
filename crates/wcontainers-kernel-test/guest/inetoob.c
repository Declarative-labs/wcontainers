// Urgent flags on internet sockets with no urgent data, and the lengths UDP refuses first.
#include <stdio.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <errno.h>
#include <string.h>
#include <unistd.h>
static void r(const char*w,long v){printf("%s: %ld %s\n",w,v,v<0?strerror(errno):"");}
int main(void){char b[80000]={0};
int u=socket(AF_INET,SOCK_DGRAM,0);struct sockaddr_in a={.sin_family=AF_INET,.sin_port=htons(5099),.sin_addr.s_addr=htonl(0x7f000001)};bind(u,(void*)&a,sizeof a);
r("udp recv oob empty",recv(u,b,10,MSG_OOB|MSG_DONTWAIT));r("udp send oob",sendto(u,"ab",2,MSG_OOB,(void*)&a,sizeof a));
sendto(u,"cd",2,0,(void*)&a,sizeof a);r("udp recv oob data",recv(u,b,10,MSG_OOB|MSG_DONTWAIT));r("udp recv after",recv(u,b,10,MSG_DONTWAIT));
r("udp send 65536",sendto(u,b,65536,0,0,0));r("udp send 65508 to",sendto(u,b,65508,0,(void*)&a,sizeof a));r("udp send 65536 oob",sendto(u,b,65536,MSG_OOB,0,0));
int u6=socket(AF_INET6,SOCK_DGRAM,0);struct sockaddr_in6 a6={.sin6_family=AF_INET6,.sin6_port=htons(5099),.sin6_addr=IN6ADDR_LOOPBACK_INIT};bind(u6,(void*)&a6,sizeof a6);
r("udp6 recv oob empty",recv(u6,b,10,MSG_OOB|MSG_DONTWAIT));r("udp6 send oob",sendto(u6,"ab",2,MSG_OOB,(void*)&a6,sizeof a6));r("udp6 send oob noaddr",send(u6,"ab",2,MSG_OOB));
r("udp6 send 70000 noaddr",send(u6,b,70000,0));r("udp6 send 70000",sendto(u6,b,70000,0,(void*)&a6,sizeof a6));r("udp6 send 65528",sendto(u6,b,65528,0,(void*)&a6,sizeof a6));
sendto(u6,"cd",2,0,(void*)&a6,sizeof a6);r("udp6 recv oob data",recv(u6,b,10,MSG_OOB|MSG_DONTWAIT));
int t=socket(AF_INET,SOCK_STREAM,0);r("tcp recv oob fresh",recv(t,b,10,MSG_OOB|MSG_DONTWAIT));listen(t,1);r("tcp recv oob listening",recv(t,b,10,MSG_OOB|MSG_DONTWAIT));
int c=socket(AF_INET,SOCK_STREAM,0);struct sockaddr_in la;socklen_t ll=sizeof la;getsockname(t,(void*)&la,&ll);la.sin_addr.s_addr=htonl(0x7f000001);connect(c,(void*)&la,sizeof la);int s=accept(t,0,0);
r("tcp recv oob connected",recv(s,b,10,MSG_OOB|MSG_DONTWAIT));write(c,"xy",2);usleep(10000);r("tcp recv oob with data",recv(s,b,10,MSG_OOB|MSG_DONTWAIT));
r("tcp send oob 0",send(c,"",0,MSG_OOB));
return 0;}

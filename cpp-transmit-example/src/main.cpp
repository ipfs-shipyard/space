#include <arpa/inet.h>
#include <iostream>
#include <unistd.h>

#include "api.hpp"

int main(int argc, char *argv[])
{
    if (argc != 4)
    {
        printf("Please provide three arguments: [ipfs_addr] [path_to_transmit] [destination_addr]\n");
        return -1;
    }
    printf("Sending {\"Transmit\": {\"path\": %s, \"addr\": %s}} to %s\n", argv[2], argv[3], argv[1]);

    // Parse out network address
    std::string addr(argv[1]);
    int split_pos = addr.find(":");
    if (split_pos == std::string::npos)
    {
        printf("Invalid address found %s", addr.c_str());
        return -1;
    }
    std::string ip = addr.substr(0, split_pos);
    std::string port = addr.substr(split_pos + 1);

    // Call into Rust code to generate transmit message
    unsigned char msg[1024];
    int len = generate_transmit_msg((unsigned char *)msg, argv[2], argv[3]);

    // Send transmit over udp to ipfs instance
    int sockfd;
    char buffer[1024];
    struct sockaddr_in servaddr;

    if ((sockfd = socket(AF_INET, SOCK_DGRAM, 0)) < 0)
    {
        perror("Socket creation failed");
        exit(-1);
    }

    memset(&servaddr, 0, sizeof(servaddr));
    servaddr.sin_family = AF_INET;
    servaddr.sin_port = htons(std::stoi(port));
    servaddr.sin_addr.s_addr = inet_addr(ip.c_str());

    sendto(sockfd, msg, len, 0, (const struct sockaddr *)&servaddr, sizeof(servaddr));
    close(sockfd);
    return 0;
}
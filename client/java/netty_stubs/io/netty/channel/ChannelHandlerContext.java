package io.netty.channel;

public interface ChannelHandlerContext {
    ChannelHandlerContext fireChannelRead(Object msg);
    ChannelHandlerContext fireExceptionCaught(Throwable cause);
}

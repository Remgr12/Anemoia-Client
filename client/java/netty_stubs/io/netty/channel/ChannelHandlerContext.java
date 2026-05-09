package io.netty.channel;
import java.net.SocketAddress;
public interface ChannelHandlerContext {
    ChannelHandlerContext fireChannelRegistered();
    ChannelHandlerContext fireChannelActive();
    ChannelHandlerContext fireChannelRead(Object msg);
    ChannelHandlerContext fireExceptionCaught(Throwable cause);
    ChannelHandlerContext write(Object msg, ChannelPromise promise);
    ChannelHandlerContext disconnect(ChannelPromise promise);
    ChannelHandlerContext close(ChannelPromise promise);
    ChannelHandlerContext deregister(ChannelPromise promise);
    ChannelHandlerContext read();
    ChannelHandlerContext flush();
}

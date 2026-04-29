package io.netty.channel;

public interface ChannelHandler {
    @interface Sharable {}
    void handlerAdded(ChannelHandlerContext ctx) throws Exception;
    void handlerRemoved(ChannelHandlerContext ctx) throws Exception;
}

package io.netty.channel;

public class ChannelInboundHandlerAdapter implements ChannelHandler {
    public void handlerAdded(ChannelHandlerContext ctx) throws Exception {}
    public void handlerRemoved(ChannelHandlerContext ctx) throws Exception {}
    public void channelRead(ChannelHandlerContext ctx, Object msg) throws Exception {
        ctx.fireChannelRead(msg);
    }
    public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) throws Exception {
        ctx.fireExceptionCaught(cause);
    }
}

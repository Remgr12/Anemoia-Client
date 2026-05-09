import io.netty.channel.ChannelDuplexHandler;
import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelPromise;

@ChannelHandler.Sharable
public class AnemoiaInterceptor extends ChannelDuplexHandler {

    // Called for each decoded inbound (server→client) packet.
    public static native void onIncoming(Object packet);

    // Called before each outbound (client→server) packet write.
    // Returns true to cancel (drop) the packet.
    public static native boolean onOutgoing(Object packet);

    @Override
    public void channelRead(ChannelHandlerContext ctx, Object msg) throws Exception {
        try {
            onIncoming(msg);
        } catch (Throwable ignored) {}
        ctx.fireChannelRead(msg);
    }

    @Override
    public void write(ChannelHandlerContext ctx, Object msg, ChannelPromise promise) throws Exception {
        boolean cancel = false;
        try {
            cancel = onOutgoing(msg);
        } catch (Throwable ignored) {}
        if (!cancel) {
            ctx.write(msg, promise);
        } else {
            promise.setSuccess();
        }
    }

    // Suppress lifecycle events fired when we inject into an already-active pipeline.
    // Without these overrides, Netty fires channelRegistered/channelActive on the new
    // handler immediately after addAfter(), which propagates to MC's packet handler
    // and triggers a spurious disconnect.
    @Override
    public void channelRegistered(ChannelHandlerContext ctx) throws Exception {}

    @Override
    public void channelActive(ChannelHandlerContext ctx) throws Exception {}

    @Override
    public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) throws Exception {}
}

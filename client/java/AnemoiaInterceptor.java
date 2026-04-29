import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;

// Compiled against stubs; loaded at runtime via MC's classloader which has real Netty.
@ChannelHandler.Sharable
public class AnemoiaInterceptor extends ChannelInboundHandlerAdapter {

    // Implemented natively in Rust via JNI registerNatives.
    public static native void onIncoming(Object packet);

    @Override
    public void channelRead(ChannelHandlerContext ctx, Object msg) throws Exception {
        try {
            onIncoming(msg);
        } catch (Throwable ignored) {
            // Never let interceptor errors kill the pipeline.
        }
        ctx.fireChannelRead(msg);
    }
}

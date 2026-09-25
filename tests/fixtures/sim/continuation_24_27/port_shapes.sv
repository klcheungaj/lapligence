// llg-test-fixture: output value links and ref/modport identity stay distinct.
`ifndef CONTINUATION_PORT_W
`define CONTINUATION_PORT_W 65
`endif
typedef logic [`CONTINUATION_PORT_W-1:0] port_lane_t;
typedef port_lane_t port_row_t [-1:-2];
typedef struct { port_lane_t tag; port_row_t lanes; } port_packet_t;
module row_output(input port_row_t source, output port_lane_t result [5:6]);
    assign result = source;
endmodule
module packet_output(input port_packet_t source, output port_packet_t result);
    assign result = source;
endmodule
module lane_output(input port_lane_t source, output port_lane_t result);
    assign result = source;
endmodule
module ref_worker(ref port_packet_t value, input logic tick, output port_lane_t observed);
    assign observed = value.lanes[-2];
    always @(posedge tick) value.lanes[-2] = value.lanes[-1];
endmodule
module ref_forward(ref port_packet_t value, input logic tick, output port_lane_t observed);
    ref_worker inner(.value(value), .tick(tick), .observed(observed));
endmodule
interface shape_bus;
    port_row_t source, result;
    modport sink(input source, output result);
endinterface
module modport_output(shape_bus.sink bus);
    assign bus.result = bus.source;
endmodule
module tb;
    localparam W = `CONTINUATION_PORT_W;
    port_row_t source;
    port_lane_t slice_result [2:-1];
    port_packet_t packet_source, packet_result, shared;
    port_lane_t scalar_source [3:2], scalar_result [3:2];
    port_lane_t observed;
    logic tick;
    shape_bus bus();
    row_output sliced(.source(source), .result(slice_result[1:0]));
    packet_output packets(.source(packet_source), .result(packet_result));
    lane_output distributed[1:0](scalar_source, scalar_result);
    ref_forward referenced(.value(shared), .tick(tick), .observed(observed));
    modport_output modported(bus);
    task automatic check;
        if (slice_result[1] !== source[-1] || slice_result[0] !== source[-2])
            $fatal(1,"output slice declaration correspondence");
        if (slice_result[2] !== port_lane_t'(1) || slice_result[-1] !== '0)
            $fatal(1,"unrelated output neighbors overwritten");
        if (packet_result !== packet_source) $fatal(1,"record output copy");
        if (scalar_result !== scalar_source) $fatal(1,"instance array distribution");
        if (bus.result !== bus.source) $fatal(1,"modport array forwarding");
        if (observed !== shared.lanes[-2]) $fatal(1,"transitive ref read");
    endtask
    initial begin
        tick=0; source='{'1,'0}; slice_result[2]=1; slice_result[-1]=0;
        packet_source.tag=1; packet_source.lanes='{'x,'z};
        shared.tag=0; shared.lanes='{'1,'0};
        scalar_source='{'1,'0}; bus.source='{'x,'z};
        #1; check();
        source[-1]='z; packet_source.lanes[-2]=1;
        scalar_source[3]='x; bus.source[-1]=1;
        #1; check();
        tick=1; #1; check();
        if (shared.lanes[-2] !== '1 || shared.tag !== '0)
            $fatal(1,"ref became a copied port value");
        tick=0; shared.lanes[-1]=0; #1; tick=1; #1; check();
        if (shared.lanes[-2] !== '0) $fatal(1,"ref forwarding retained stale storage");
        $display("PORT_SHAPES_PASS W=%0d",W); $finish(0);
    end
endmodule

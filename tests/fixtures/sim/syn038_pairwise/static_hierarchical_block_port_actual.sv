`default_nettype none
// IEEE 1800-2009 §§6.21, 23.3, and 23.6: static block storage is persistent,
// named block items can be referenced hierarchically, and an input port actual
// may use that hierarchical expression.
module static_hier_port_child(input logic [7:0] value);
    logic [7:0] seen;
    always_comb seen = value;
endmodule

module tb;
    // SYN038-GAP-CO-port_actual__SL-static_local.
    static_hier_port_child primary(.value(tb.primary_process.static_value));
    // Same-named sibling storage is a hierarchy-resolution control.
    static_hier_port_child sibling(.value(tb.sibling_process.static_value));

    initial begin : primary_process
        static logic [7:0] static_value;
        static_value = 8'h31;
    end

    initial begin : sibling_process
        static logic [7:0] static_value;
        static_value = 8'h42;
    end

    initial begin
        #1;
        if (primary.seen !== 8'h31 || sibling.seen !== 8'h42)
            $fatal(1, "hierarchical static port actual mismatch: %h %h",
                   primary.seen, sibling.seen);
        $display("static_hierarchical_block_port_actual=passed");
        $finish;
    end
endmodule
`default_nettype wire

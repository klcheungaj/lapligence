// llg-test-fixture: independent multi-output primitive terminals, fixed selectors.
`ifndef CONTINUATION_PORT_W
`define CONTINUATION_PORT_W 65
`endif
module tb;
    localparam W = `CONTINUATION_PORT_W;
    typedef struct packed { logic [W-1:0] high, low; } pair_t;
    wire pair_t result;
    reg [W-1:0] source, conflict;
    wire [W-1:0] distributed;
    buf selected[W-1:0](result.high, result.low, source);
    assign result.high = conflict;
    buf lanes[W-1:0](distributed,source);
    initial begin
        source=0; source[0]=1; conflict={W{1'bz}};
        #1;
        if (result.high !== source || result.low !== source || distributed !== source)
            $fatal(1,"primitive output/member/instance ordering");
        conflict=~source; #1;
        if (result.high !== {W{1'bx}} || result.low !== source)
            $fatal(1,"independent output contributions overwritten");
        conflict={W{1'bz}}; source=~source; #1;
        if (result.high !== source || result.low !== source || distributed !== source)
            $fatal(1,"release or sibling terminal stale");
        $display("TERMINALS_PASS W=%0d",W); $finish(0);
    end
endmodule

// llg-test-fixture: SV 10.11 and 11.5.1 indexed aliases pair bits MSB-to-LSB.
`ifndef CONTINUATION_ALIAS_W
`define CONTINUATION_ALIAS_W 65
`endif
module tb;
    localparam W = `CONTINUATION_ALIAS_W;
    wire [0:W+1] up;
    wire [W+1:0] down;
    wire [W-1:0] u_minus, u_plus, d_minus, d_plus;
    typedef struct packed { logic [0:W+1] payload; logic flag; } record_t;
    wire record_t record_net;
    wire [W-1:0] member;
    wire [W-1:0] rows [1:0];
    wire [W-1:0] row_alias;
    logic raw_enable, alias_enable;
    logic [W-1:0] raw, back;
    alias u_minus = up[W -: W];
    alias u_plus = up[1 +: W];
    alias d_minus = down[W -: W];
    alias d_plus = down[1 +: W];
    // The pinned frontend rejects member selects in a net alias, so alias the
    // same packed-struct bits: payload[W -: W] of [0:W+1] is record_net[W+1:2].
    alias member = record_net[W+1:2];
    alias row_alias = rows[0];
    assign up = raw_enable ? {1'b1,raw,1'b0} : 'z;
    assign down = raw_enable ? {1'b1,raw,1'b0} : 'z;
    assign record_net.payload = raw_enable ? {1'b1,raw,1'b0} : 'z;
    assign record_net.flag = 1'b1;
    assign rows[0] = raw_enable ? raw : 'z;
    assign rows[1] = '0;
    assign u_minus = alias_enable ? back : 'z;
    assign d_minus = alias_enable ? back : 'z;
    assign member = alias_enable ? back : 'z;
    assign row_alias = alias_enable ? back : 'z;
    task automatic check(input logic [W-1:0] expected);
        if (u_minus !== expected || u_plus !== expected || d_minus !== expected || d_plus !== expected)
            $fatal(1,"indexed alias reversed or lost bits");
        if (member !== expected || row_alias !== expected || rows[0] !== expected)
            $fatal(1,"member/array alias lost identity");
        if (rows[1] !== '0 || record_net.flag !== 1'b1) $fatal(1,"alias changed its neighbor");
    endtask
    initial begin
        raw='0; raw[0]=1; back='0; raw_enable=1; alias_enable=0;
        #1; check(raw);
        raw=~raw; #1; check(raw);
        raw_enable=0; alias_enable=1; back='0; back[0]=1;
        #1; check(back);
        for (int n=0; n<W; n++) begin
            if (up[1+n] !== back[W-1-n] || down[W-n] !== back[W-1-n])
                $fatal(1,"backward alias connectivity");
        end
        force u_minus[0]=0; #1;
        if (up[W] !== 0 || u_plus[0] !== 0) $fatal(1,"forced alias visibility");
        release u_minus[0]; #1; check(back);
        alias_enable=0; #1; check('z);
        if (up !== 'z || down !== 'z) $fatal(1,"stale alias contribution after release");
        $display("ALIASES_PASS W=%0d",W); $finish(0);
    end
endmodule

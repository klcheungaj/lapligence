// SIM-040 A02: marshalling round trips keep X and Z in every 4-state
// position and the declared dimensions (SV H.7.7, H.10.1.2). Aliased
// actuals and a foreign string result follow the specified copy order
// (35.6.1, decisions S40-D1 and S40-D2): every input is copied in before the call,
// outputs are copied out in declaration order after it, and the result is
// assigned last. Build roundtrip_xz.c into a shared library and pass it with
// --dpi-lib.
module tb;
    import "DPI-C" function void rt_logic(input logic [69:0] i, output logic [69:0] o,
                                          inout logic [69:0] io);
    import "DPI-C" function void rt_init(output logic [40:0] lo, output bit [40:0] bo,
                                         output logic [3:0] la [2], output bit [3:0] ba [2]);
    import "DPI-C" function void rt_alias(input bit [39:0] i, output bit [39:0] o,
                                          inout bit [39:0] io);
    import "DPI-C" function int rt_result(output int o);
    import "DPI-C" function string rt_string(input string s, output string o);
    import "DPI-C" task rt_task(input int i, output logic [69:0] o);
    import "DPI-C" function void rt_wide(inout logic [4999:0] v, output bit [4100:0] b);
    import "DPI-C" function void rt_scalar(input logic a, output logic b, inout logic c);

    logic [69:0] li, lo, lio;
    logic [40:0] init_l;
    bit [40:0] init_b;
    logic [3:0] init_la [2];
    bit [3:0] init_ba [2];
    bit [39:0] x, y;
    bit [7:0] mem [4];
    int k, r;
    string s;
    logic [4999:0] v;
    bit [4100:0] b;
    logic sa, sb, sc;

    initial begin
        li = {6'b10xz01, 32'hxxxx_0000, 32'h1234_zzzz};
        lo = 70'h0;
        lio = {6'bz1x0zx, 32'h0123_4567, 32'hzzzz_xxxx};
        rt_logic(li, lo, lio);
        $display("lo=%b_%h_%h", lo[69:64], lo[63:32], lo[31:0]);
        $display("lio=%b_%h_%h", lio[69:64], lio[63:32], lio[31:0]);

        init_l = '0;
        init_b = '1;
        rt_init(init_l, init_b, init_la, init_ba);
        $display("init l=%h b=%h la=%b,%b ba=%b,%b", init_l, init_b, init_la[0], init_la[1],
                 init_ba[0], init_ba[1]);

        x = 40'h12_3456_789a;
        rt_alias(x, x, x);
        $display("alias x=%h", x);
        y = 40'h1;
        k = 2;
        rt_alias(40'hff_0000_0001, mem[k][7:0], y);
        $display("select mem=%h,%h,%h,%h y=%h", mem[0], mem[1], mem[2], mem[3], y);

        r = 5;
        r = rt_result(r);
        $display("result r=%0d", r);

        s = "in";
        s = rt_string(s, s);
        $display("string s=%s", s);

        rt_task(5, lo);
        $display("task lo=%b_%h_%h", lo[69:64], lo[63:32], lo[31:0]);

        v = {4996'b0, 4'b10xz};
        v[4999] = 1'bz;
        v[2500] = 1'bx;
        rt_wide(v, b);
        $display("wide v=%b..%b..%b b=%b..%h", v[4999:4996], v[2501:2499], v[3:0], b[4100],
                 b[31:0]);

        sa = 1'bz;
        sc = 1'bx;
        rt_scalar(sa, sb, sc);
        $display("scalar sb=%b sc=%b", sb, sc);
        $finish;
    end
endmodule

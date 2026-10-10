// SIM-040 A01: packed formals of every width class cross the DPI as
// canonical 32-bit chunks (SV H.7.7), inputs by const pointer (H.8.7),
// outputs and inouts by pointer (H.8.8). Build packed_widths.c into a shared
// library and pass it with --dpi-lib.
module tb;
    typedef struct packed { bit [3:0] hi; logic [3:0] lo; } ps_t;
    typedef union packed { bit [7:0] raw; ps_t parts; } pu_t;
    typedef enum bit [2:0] { E0, E5 = 5 } e3_t;

    import "DPI-C" function void pw_in(input bit [1:0] w2, input bit [30:0] w31,
        input bit [31:0] w32, input bit [32:0] w33, input bit signed [63:0] w64,
        input bit [64:0] w65, input logic [95:0] l96, input logic [128:0] l129);
    import "DPI-C" function void pw_out(output bit [32:0] o33,
        output logic [63:0] o64, output bit signed [64:0] o65,
        output logic [127:0] o128);
    import "DPI-C" function void pw_inout(inout bit [32:0] x33,
        inout logic [96:0] x97);
    import "DPI-C" function void pw_kinds(input integer i, input time t,
        input ps_t ps, input pu_t pu, input e3_t e, output integer oi,
        output time ot);

    bit [32:0] o33;
    logic [63:0] o64;
    bit signed [64:0] o65;
    logic [127:0] o128;
    bit [32:0] x33;
    logic [96:0] x97;
    ps_t ps;
    pu_t pu;
    integer oi;
    time ot;

    initial begin
        pw_in(2'b10, 31'h7fff_fffe, 32'hdead_beef, 33'h1_0000_0003, -64'sd2,
            65'h1_8000_0000_0000_0001,
            {32'h0123_4567, 32'hxxxx_0000, 32'hzzzz_ffff}, {1'bx, 128'h0});
        pw_out(o33, o64, o65, o128);
        $display("o33=%h o64=%h_%b o65=%0d", o33, o64[63:32], o64[3:0], o65);
        $display("o128=%b_%h", o128[127:124], o128[123:0]);
        x33 = 33'h1_2345_6789;
        x97 = {1'b0, 32'hxxxx_xxxx, 64'h0000_0001_ffff_ffff};
        pw_inout(x33, x97);
        $display("x33=%h x97=%h", x33, x97);
        ps = {4'ha, 4'bx01z};
        pu.raw = 8'h3c;
        pw_kinds(-5, 64'd5_000_000_000, ps, pu, E5, oi, ot);
        $display("oi=%h_%b ot=%h", oi[31:8], oi[7:0], ot);
        $finish;
    end
endmodule

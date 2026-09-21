// llg-test-fixture: SYN-019 legacy Verilog-2001 forms and builtin controls
// IEEE 1364-2001 §§6.2, 6.2.1, 9.7.5, and 12.3: ANSI and non-ANSI ports,
// declaration initialization, @* sensitivity, and standard system names.
module nonansi(a, b);
    input a;
    output b;
    assign b = a;
endmodule

module ansi(input a, output b);
    assign b = a;
endmodule

module tb;
    reg in;
    reg combinational;
    wire out_legacy;
    wire out_ansi;
    reg initialized = in;

    nonansi u0(in, out_legacy);
    ansi u1(.a(in), .b(out_ansi));

    always @* combinational = in;

    initial begin
        in = 1'b1;
        #0;
        $display("legacy=%b ansi=%b comb=%b signed=%0d init=%b", out_legacy,
                 out_ansi, combinational, $signed(4'b1111), initialized);
        $finish;
    end
endmodule

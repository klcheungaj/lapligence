// IEEE 1800-2009 6.11.2, 6.24.1, Table 6-7: uninitialized four-state
// variables read X, two-state variables read zero, and conversion clears
// X/Z only at the typed two-state boundary. Widths cross every 64-bit limb
// boundary and include one multi-limb width; the expected trace is an
// independent Rust bit-string oracle in tests/sim_g1_closure.rs.
module tb;
    logic d4_1; bit d2_1; logic c4_1; bit c2_1;
    logic signed s4_1; bit signed s2_1;

    logic [30:0] d4_31; bit [30:0] d2_31; logic [30:0] c4_31; bit [30:0] c2_31;
    logic signed [30:0] s4_31; bit signed [30:0] s2_31;

    logic [31:0] d4_32; bit [31:0] d2_32; logic [31:0] c4_32; bit [31:0] c2_32;
    logic signed [31:0] s4_32; bit signed [31:0] s2_32;

    logic [32:0] d4_33; bit [32:0] d2_33; logic [32:0] c4_33; bit [32:0] c2_33;
    logic signed [32:0] s4_33; bit signed [32:0] s2_33;

    logic [62:0] d4_63; bit [62:0] d2_63; logic [62:0] c4_63; bit [62:0] c2_63;
    logic signed [62:0] s4_63; bit signed [62:0] s2_63;

    logic [63:0] d4_64; bit [63:0] d2_64; logic [63:0] c4_64; bit [63:0] c2_64;
    logic signed [63:0] s4_64; bit signed [63:0] s2_64;

    logic [64:0] d4_65; bit [64:0] d2_65; logic [64:0] c4_65; bit [64:0] c2_65;
    logic signed [64:0] s4_65; bit signed [64:0] s2_65;

    logic [128:0] d4_129; bit [128:0] d2_129; logic [128:0] c4_129; bit [128:0] c2_129;
    logic signed [128:0] s4_129; bit signed [128:0] s2_129;

    logic [255:0] d4_256; bit [255:0] d2_256; logic [255:0] c4_256; bit [255:0] c2_256;
    logic signed [255:0] s4_256; bit signed [255:0] s2_256;

    logic [7:0] xsrc;

    initial begin
        xsrc = 8'bz1x0z1x0;
        c4_1 = xsrc; c2_1 = xsrc; s4_1 = 4'sb1x0z; s2_1 = 4'sb1x0z;
        c4_31 = xsrc; c2_31 = xsrc; s4_31 = 4'sb1x0z; s2_31 = 4'sb1x0z;
        c4_32 = xsrc; c2_32 = xsrc; s4_32 = 4'sb1x0z; s2_32 = 4'sb1x0z;
        c4_33 = xsrc; c2_33 = xsrc; s4_33 = 4'sb1x0z; s2_33 = 4'sb1x0z;
        c4_63 = xsrc; c2_63 = xsrc; s4_63 = 4'sb1x0z; s2_63 = 4'sb1x0z;
        c4_64 = xsrc; c2_64 = xsrc; s4_64 = 4'sb1x0z; s2_64 = 4'sb1x0z;
        c4_65 = xsrc; c2_65 = xsrc; s4_65 = 4'sb1x0z; s2_65 = 4'sb1x0z;
        c4_129 = xsrc; c2_129 = xsrc; s4_129 = 4'sb1x0z; s2_129 = 4'sb1x0z;
        c4_256 = xsrc; c2_256 = xsrc; s4_256 = 4'sb1x0z; s2_256 = 4'sb1x0z;

        $display("d4_1=%b", d4_1);
        $display("d2_1=%b", d2_1);
        $display("c4_1=%b", c4_1);
        $display("c2_1=%b", c2_1);
        $display("s4_1=%b", s4_1);
        $display("s2_1=%b", s2_1);

        $display("d4_31=%b", d4_31);
        $display("d2_31=%b", d2_31);
        $display("c4_31=%b", c4_31);
        $display("c2_31=%b", c2_31);
        $display("s4_31=%b", s4_31);
        $display("s2_31=%b", s2_31);

        $display("d4_32=%b", d4_32);
        $display("d2_32=%b", d2_32);
        $display("c4_32=%b", c4_32);
        $display("c2_32=%b", c2_32);
        $display("s4_32=%b", s4_32);
        $display("s2_32=%b", s2_32);

        $display("d4_33=%b", d4_33);
        $display("d2_33=%b", d2_33);
        $display("c4_33=%b", c4_33);
        $display("c2_33=%b", c2_33);
        $display("s4_33=%b", s4_33);
        $display("s2_33=%b", s2_33);

        $display("d4_63=%b", d4_63);
        $display("d2_63=%b", d2_63);
        $display("c4_63=%b", c4_63);
        $display("c2_63=%b", c2_63);
        $display("s4_63=%b", s4_63);
        $display("s2_63=%b", s2_63);

        $display("d4_64=%b", d4_64);
        $display("d2_64=%b", d2_64);
        $display("c4_64=%b", c4_64);
        $display("c2_64=%b", c2_64);
        $display("s4_64=%b", s4_64);
        $display("s2_64=%b", s2_64);

        $display("d4_65=%b", d4_65);
        $display("d2_65=%b", d2_65);
        $display("c4_65=%b", c4_65);
        $display("c2_65=%b", c2_65);
        $display("s4_65=%b", s4_65);
        $display("s2_65=%b", s2_65);

        $display("d4_129=%b", d4_129);
        $display("d2_129=%b", d2_129);
        $display("c4_129=%b", c4_129);
        $display("c2_129=%b", c2_129);
        $display("s4_129=%b", s4_129);
        $display("s2_129=%b", s2_129);

        $display("d4_256=%b", d4_256);
        $display("d2_256=%b", d2_256);
        $display("c4_256=%b", c4_256);
        $display("c2_256=%b", c2_256);
        $display("s4_256=%b", s4_256);
        $display("s2_256=%b", s2_256);

        $display("PASS integral_default_matrix");
        $finish(0);
    end
endmodule

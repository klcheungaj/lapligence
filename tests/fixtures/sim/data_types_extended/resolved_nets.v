// IEEE 1364-2001 3.7 and 7.13: wand/wor combine drivers with AND/OR truth
// tables, tri resolves ordinary drivers, and tri0/tri1 provide pull defaults.
module tb;
    parameter WIDTH = 2048;
    reg [WIDTH-1:0] drive_a;
    reg [WIDTH-1:0] drive_b;
    wand [WIDTH-1:0] and_net;
    wor [WIDTH-1:0] or_net;
    tri [WIDTH-1:0] tri_net;
    tri0 [WIDTH-1:0] pull_zero;
    tri1 [WIDTH-1:0] pull_one;
    reg [WIDTH-1:0] expected_and;
    reg [WIDTH-1:0] expected_or;
    reg [WIDTH-1:0] expected_tri;
    integer failed;

    assign and_net = drive_a;
    assign and_net = drive_b;
    assign or_net = drive_a;
    assign or_net = drive_b;
    assign tri_net = drive_a;
    assign tri_net = drive_b;

    initial begin
        failed = 0;
        drive_a = {WIDTH{1'bz}};
        drive_b = {WIDTH{1'bz}};
        #1;
        if (and_net !== {WIDTH{1'bz}} || or_net !== {WIDTH{1'bz}} ||
            tri_net !== {WIDTH{1'bz}} || pull_zero !== '0 || pull_one !== '1) begin
            $display("FAIL undriven-and-pulls WIDTH=%0d", WIDTH);
            failed = 1;
        end

        drive_a = '1;
        drive_b = {WIDTH{1'bz}};
        #1;
        if (!failed && (and_net !== '1 || or_net !== '1 || tri_net !== '1)) begin
            $display("FAIL z-identity WIDTH=%0d", WIDTH);
            failed = 1;
        end

        drive_a = '0;
        drive_b = '1;
        #1;
        if (!failed && (and_net !== '0 || or_net !== '1 ||
                        tri_net !== {WIDTH{1'bx}})) begin
            $display("FAIL conflicting-drivers WIDTH=%0d", WIDTH);
            failed = 1;
        end

        drive_a = {WIDTH{1'bz}};
        drive_b = {WIDTH{1'bz}};
        drive_a[WIDTH-1] = 1'b1;
        drive_b[WIDTH-1] = 1'b0;
        drive_a[64] = 1'bx;
        drive_b[64] = 1'b1;
        expected_and = {WIDTH{1'bz}};
        expected_or = {WIDTH{1'bz}};
        expected_tri = {WIDTH{1'bz}};
        expected_and[WIDTH-1] = 1'b0;
        expected_or[WIDTH-1] = 1'b1;
        expected_tri[WIDTH-1] = 1'bx;
        expected_and[64] = 1'bx;
        expected_or[64] = 1'b1;
        expected_tri[64] = 1'bx;
        #1;
        if (!failed && (and_net !== expected_and || or_net !== expected_or ||
                        tri_net !== expected_tri)) begin
            $display("FAIL positional-resolution WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS resolved_nets WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule

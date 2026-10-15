// llg-test-fixture: SIM-042 A01 object model. A VPI application (objects.c,
// task $walk) traverses modules, generate scopes, named blocks, ports,
// parameters, variables, nets, arrays, elements and selects; checks types,
// names and bounds; reads X/Z and real values; and writes selected
// destinations. Expected output: objects.out, derived in readme.md.
module leaf #(parameter int W = 4, parameter logic [7:0] P = 8'b1010_x1z0) (
    input logic [W-1:0] a,
    output logic [W-1:0] y
);
    localparam int L = W * 2;
    assign y = ~a;
endmodule

module tb;
    logic [7:0] v = 8'h3c;
    logic [0:3] up = 4'b10xz;
    wire [3:0] w;
    logic [7:0] mem [0:3];
    logic signed [7:0] grid [1:0][2:0];
    real r = 1.25;
    int i = -5;
    bit [5:2] bv = 4'b1010;
    byte b = -2;
    integer ii;
    time t = 64'd100;
    leaf #(.W(4)) u1 (.a(v[3:0]), .y(w));
    for (genvar g = 0; g < 2; g++) begin : gen
        logic [1:0] gv = 2'(g);
    end
    initial begin : main
        logic [2:0] local_v;
        local_v = 3'b101;
        mem[0] = 8'h11;
        mem[1] = 8'h22;
        mem[2] = 8'b1x0z_0000;
        mem[3] = 8'h44;
        grid[0][0] = -8'sd2;
        grid[0][1] = -8'sd1;
        grid[0][2] = 8'sd0;
        grid[1][0] = 8'sd1;
        grid[1][1] = 8'sd2;
        grid[1][2] = 8'sd3;
        #1 $walk;
        #1 $display("v=%h up=%b w=%h mem3=%h grid10=%0d r=%0.2f i=%0d bv=%b local=%b gv1=%b",
                    v, up, w, mem[3], grid[1][0], r, i, bv, local_v, gen[1].gv);
        $finish;
    end
endmodule

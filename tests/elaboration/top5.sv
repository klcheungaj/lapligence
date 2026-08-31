// Test 5: misc statements & types
module misc #(parameter W = 4) (
    input logic [W-1:0] sel,
    input logic [W-1:0] a,
    input logic [W-1:0] b,
    output logic [W-1:0] o,
    output logic [W-1:0] o2,
    output logic [W-1:0] o3,
    output integer iout,
    output real rout
);
    integer i;
    real r;
    time t;

    always @(*) begin
        casez (sel)
            4'b1???: o = a;
            4'b01??: o = b;
            default: o = 4'h0;
        endcase
    end

    always @(a) begin
        o2 = 0;
        repeat (2) o2 = o2 + a;
        while (o2 > 16) o2 = o2 >> 1;
    end

    always @(a) begin
        i = 0;
        iout = i;
        r = 1.5;
        rout = r;
        t = $time;
    end
endmodule

module order_inst (
    input logic [3:0] a,
    output logic [3:0] o
);
    misc #(4) u_misc (
        .sel(a), .a(a), .b(a), .o(o), .o2(), .o3(), .iout(), .rout()
    );
endmodule

module top5 (
    input logic [3:0] a,
    output logic [3:0] o
);
    order_inst u_oi (.a(a), .o(o));
endmodule

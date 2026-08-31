// Test 4: interfaces, modports, always_comb, structs
interface bus_if #(parameter int W = 8);
    logic [W-1:0] data;
    logic valid;
    modport master (output data, output valid);
    modport slave  (input data, input valid);
endinterface

typedef struct packed {
    logic [3:0] a;
    logic [3:0] b;
} pair_t;

module ifc_master (
    bus_if.master m
);
    always_comb begin
        m.data = m.data + 1;
        m.valid = 1;
    end
endmodule

module struct_mod (
    input  logic [7:0] din,
    output logic [7:0] dout
);
    pair_t p;
    always_comb begin
        p.a = din[7:4];
        p.b = din[3:0];
        dout = {p.b, p.a};
    end
endmodule

module top4 (
    input logic clk,
    output logic [7:0] dout
);
    bus_if #(.W(8)) u_bus ();
    ifc_master u_m (.m(u_bus.master));
    struct_mod u_s (.din(u_bus.data), .dout(dout));
    assign u_bus.data = 8'h2A;
endmodule

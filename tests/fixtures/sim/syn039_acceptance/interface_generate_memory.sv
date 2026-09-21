// llg-test-fixture: SYN-039 parameterized interface/generate memory.
// IEEE 1800-2009 §§25.3, 25.5, 27. A finite generate-for creates independent
// parameterized interface instances, each connected to a finite initialized
// memory block.
interface memory_bus #(
    parameter int WIDTH = 8,
    parameter int ID = 0
);
    logic [1:0] address;
    logic [WIDTH-1:0] data;
    modport bank (input address, output data);
endinterface

module memory_bank #(
    parameter int ID = 0
) (
    memory_bus.bank bus,
    output logic [7:0] value
);
    localparam logic [7:0] ID_VALUE = ID;
    logic [7:0] memory [0:3];

    initial begin
        memory[0] = 8'h10 + ID_VALUE;
        memory[1] = 8'h20 + ID_VALUE;
        memory[2] = 8'h30 + ID_VALUE;
        memory[3] = 8'h40 + ID_VALUE;
    end

    always_comb begin
        value = memory[bus.address] + ID_VALUE;
        bus.data = value;
    end
endmodule

module tb;
    logic [7:0] values [0:1];
    genvar index;
    generate
        for (index = 0; index < 2; index = index + 1) begin : generated_bank
            memory_bus #(.WIDTH(8), .ID(index)) u_bus();
            memory_bank #(.ID(index)) u_bank(
                .bus(u_bus.bank),
                .value(values[index])
            );
        end
    endgenerate

    initial begin
        generated_bank[0].u_bus.address = 2'd0;
        generated_bank[1].u_bus.address = 2'd1;
        #1;
        $display("first=%h/%h bus=%h/%h", values[0], values[1],
                 generated_bank[0].u_bus.data, generated_bank[1].u_bus.data);
        generated_bank[0].u_bus.address = 2'd3;
        generated_bank[1].u_bus.address = 2'd2;
        #1;
        $display("second=%h/%h bus=%h/%h", values[0], values[1],
                 generated_bank[0].u_bus.data, generated_bank[1].u_bus.data);
        $finish(0);
    end
endmodule

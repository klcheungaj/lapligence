// llg-test-fixture: tests/fixtures/sim/array_sensitivity/fixed_array.sv
// IEEE 1800-2009 §§7.4, 9.4.2, 9.4.3 and 10.3: fixed-array element writes
// wake continuous, implicit, explicit and level-sensitive readers.
module child(input logic [7:0] input_value, output logic [7:0] output_value);
    assign output_value = input_value;
endmodule

module tb;
    logic [7:0] mem [0:3];
    logic [1:0] index;
    wire [7:0] continuous_value;
    wire [7:0] stable_value;
    wire [7:0] port_value;
    logic [7:0] explicit_value;
    logic [7:0] event_value;
    logic [7:0] combinational_value;

    assign continuous_value = mem[index];
    assign stable_value = mem[1];
    child u(mem[index], port_value);

    always @* begin
        explicit_value = mem[index];
    end

    always @(mem[index]) begin
        event_value = mem[index];
    end

    always_comb begin
        combinational_value = mem[index];
    end

    initial begin
        index = 0;
        mem[0] = 8'ha5;
        #1 $display("fixed0=%h %h %h %h %h %h", continuous_value, stable_value,
                    explicit_value, event_value, combinational_value, port_value);

        index = 2;
        mem[2] = 8'h5a;
        #1 $display("fixed1=%h %h %h %h %h %h", continuous_value, stable_value,
                    explicit_value, event_value, combinational_value, port_value);

        index = 1;
        mem[1] = 8'h3c;
        #1 $display("fixed2=%h %h %h %h %h %h", continuous_value, stable_value,
                    explicit_value, event_value, combinational_value, port_value);

        wait (mem[index] == 8'hc3);
        #0;
        $display("fixed_wait=%h %h %h %h %h %h", continuous_value, stable_value,
                 explicit_value, event_value, combinational_value, port_value);
        $finish;
    end

    initial begin
        #4 mem[1] = 8'hc3;
    end
endmodule

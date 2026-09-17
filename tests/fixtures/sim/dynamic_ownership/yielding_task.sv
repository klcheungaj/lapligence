`timescale 1ns/1ns
module tb;
    logic [64:0] value, result;
    task automatic delayed_increment(output logic [64:0] output_value,
                                      input logic [64:0] input_value);
        logic [64:0] saved;
        saved = input_value + 65'd1;
        #1;
        output_value = saved + 65'd1;
    endtask
    initial begin
        value = 65'd7;
        result = 0;
        repeat (20) begin
            delayed_increment(result, value);
            value = result;
        end
        $display("%0d", value);
        $finish(0);
    end
endmodule

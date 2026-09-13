// llg-test-fixture: tests/fixtures/sim/array_sensitivity/containers.sv
// IEEE 1800-2009 §§7.5, 7.10, 9.4.2 and 9.4.3: resizable contents and shape
// changes wake readers without retaining reallocatable element addresses.
module tb;
    logic [7:0] values[];
    logic [7:0] queue[$];
    wire [7:0] continuous_value;
    logic [7:0] combinational_value;
    logic [7:0] explicit_value;
    logic [7:0] value_count;
    logic [7:0] queue_count;

    assign continuous_value = values[0];

    always_comb begin
        combinational_value = values[0];
        value_count = values.size();
    end

    always @* begin
        explicit_value = queue[0];
        queue_count = queue.size();
    end

    initial begin
        values = new[1];
        values[0] = 8'h11;
        queue.push_back(8'h11);
        #1;
        $display("container1=%h %0d %h %0d", continuous_value,
                 value_count, combinational_value, queue_count);

        values = new[2](values);
        values[1] = 8'h22;
        queue.push_back(8'h22);
        #1 $display("container2=%h %0d %h %0d", continuous_value,
                    value_count, combinational_value, queue_count);

        queue.delete();
        values.delete();
        #1 $display("container3=%h %0d %h %0d", continuous_value,
                    value_count, combinational_value, queue_count);
        $finish;
    end

    initial begin
        wait (values.size() == 2);
        $display("wait_resize=%0d", values.size());
        wait (queue.size() == 0);
        $display("wait_delete=%0d", queue.size());
    end
endmodule

// IEEE 1800-2009 7.5 and 7.6: dynamic arrays begin empty, new[] initializes
// or resizes them, assignment copies values, and delete() returns to size zero.
module tb;
    bit [15:0] values[];
    bit [15:0] copy[];

    initial begin
        if (values.size() !== 0) begin
            $display("FAIL dynamic_array default_size");
            $finish;
        end

        values = new[3];
        values[0] = 16'd5;
        values[1] = 16'd6;
        values[2] = 16'd7;
        copy = values;
        values[1] = 16'd60;
        if (copy.size() !== 3 || copy[0] !== 16'd5 ||
            copy[1] !== 16'd6 || copy[2] !== 16'd7) begin
            $display("FAIL dynamic_array value_copy");
            $finish;
        end

        values = new[5](values);
        if (values.size() !== 5 || values[0] !== 16'd5 ||
            values[1] !== 16'd60 || values[2] !== 16'd7 ||
            values[3] !== 16'd0 || values[4] !== 16'd0) begin
            $display("FAIL dynamic_array resize_preserve");
            $finish;
        end

        values.delete();
        if (values.size() !== 0) begin
            $display("FAIL dynamic_array delete");
            $finish;
        end

        $display("PASS dynamic_array");
        $finish;
    end
endmodule

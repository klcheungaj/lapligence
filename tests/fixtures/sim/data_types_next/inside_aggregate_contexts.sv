// IEEE 1800-2009 11.4.13 and 12.5.4: inside recursively visits unpacked
// aggregate/set members, uses inclusive ranges, and compares real and string
// values with their ordinary equality and ordering rules.
module tb;
    logic [7:0] fixed [1:0];
    logic [7:0] nested [0:1][0:1];
    logic [7:0] packed_value;
    int dynamic_values[];
    int queue_values[$];
    int associative_values[int];
    real real_value;
    real real_values [0:1];
    string text;
    logic result;

    initial begin
        fixed[1] = 8'd9;
        fixed[0] = 8'd10;
        result = 8'd9 inside {fixed};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts fixed_array");
            $finish;
        end

        nested[0][0] = 8'd1;
        nested[0][1] = 8'd2;
        nested[1][0] = 8'd3;
        nested[1][1] = 8'd4;
        result = 8'd2 inside {nested};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts nested_set");
            $finish;
        end

        packed_value = 8'd5;
        result = packed_value inside {[$:8'd5]};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts packed_open_range");
            $finish;
        end

        dynamic_values = new[2];
        dynamic_values[0] = 21;
        dynamic_values[1] = 22;
        result = 22 inside {dynamic_values};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts dynamic_array");
            $finish;
        end
        dynamic_values.delete();
        result = 22 inside {dynamic_values};
        if (result !== 1'b0) begin
            $display("FAIL inside_aggregate_contexts empty_dynamic_array");
            $finish;
        end

        queue_values.push_back(31);
        queue_values.push_back(32);
        result = 31 inside {queue_values};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts queue");
            $finish;
        end

        associative_values[4] = 41;
        associative_values[8] = 42;
        result = 42 inside {associative_values};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts associative_array");
            $finish;
        end

        real_value = 1.5;
        real_values[0] = 2.5;
        real_values[1] = 3.5;
        result = 2.5 inside {real_values};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts real_array");
            $finish;
        end
        result = real_value inside {[1.0:2.0]};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts real_range");
            $finish;
        end
        result = real_value inside {[$:1.0]};
        if (result !== 1'b0) begin
            $display("FAIL inside_aggregate_contexts real_open_range");
            $finish;
        end
        case (real_value) inside
            [1.0:2.0]: result = 1'b1;
            default: result = 1'b0;
        endcase
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts real_case");
            $finish;
        end

        text = "beta";
        result = text inside {"alpha", "beta"};
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts string");
            $finish;
        end
        case (text) inside
            "alpha": result = 1'b0;
            "beta": result = 1'b1;
            default: result = 1'b0;
        endcase
        if (result !== 1'b1) begin
            $display("FAIL inside_aggregate_contexts string_case");
            $finish;
        end

        $display("PASS inside_aggregate_contexts");
        $finish;
    end
endmodule

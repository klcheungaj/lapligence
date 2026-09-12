// IEEE 1800-2009 7.12: packed-element locator, ordering, and
// reduction methods preserve queue order and evaluate each with-clause item.
module tb;
    int values[$];
    int dynamic_values[];
    int associative[int];
    int result[$];
    int total;

    initial begin
        values.push_back(1);
        values.push_back(3);
        values.push_back(2);
        values.push_back(3);
        values.push_back(4);

        associative[2] = 1;
        associative[4] = 3;
        associative[6] = 2;
        associative[8] = 3;
        associative[10] = 4;

        result = values.find() with (item > 2);
        if (result.size() != 3
                || result[0] !== 3 || result[1] !== 3 || result[2] !== 4) begin
            $display("FAIL array_methods_find");
            $finish;
        end
        result = values.find_index() with (item > 2);
        if (result.size() != 3
                || result[0] !== 1 || result[1] !== 3 || result[2] !== 4) begin
            $display("FAIL array_methods_find_index");
            $finish;
        end
        result = values.find_first() with (item > 2);
        if (result.size() != 1 || result[0] !== 3) begin
            $display("FAIL array_methods_find_first");
            $finish;
        end
        result = values.find_first_index() with (item > 2);
        if (result.size() != 1 || result[0] !== 1) begin
            $display("FAIL array_methods_find_first_index");
            $finish;
        end
        result = values.find_last() with (item > 2);
        if (result.size() != 1 || result[0] !== 4) begin
            $display("FAIL array_methods_find_last");
            $finish;
        end
        result = values.find_last_index() with (item > 2);
        if (result.size() != 1 || result[0] !== 4) begin
            $display("FAIL array_methods_find_last_index");
            $finish;
        end

        result = values.min();
        if (result.size() != 1 || result[0] !== 1) begin
            $display("FAIL array_methods_min");
            $finish;
        end
        result = values.max();
        if (result.size() != 1 || result[0] !== 4) begin
            $display("FAIL array_methods_max");
            $finish;
        end
        result = values.min() with (item * 2);
        if (result.size() != 1 || result[0] !== 1) begin
            $display("FAIL array_methods_min_with");
            $finish;
        end
        result = values.max() with (item * 2);
        if (result.size() != 1 || result[0] !== 4) begin
            $display("FAIL array_methods_max_with");
            $finish;
        end
        result = values.unique();
        if (result.size() != 4
                || result[0] !== 1 || result[1] !== 3
                || result[2] !== 2 || result[3] !== 4) begin
            $display("FAIL array_methods_unique");
            $finish;
        end
        result = values.unique_index();
        if (result.size() != 4
                || result[0] !== 0 || result[1] !== 1
                || result[2] !== 2 || result[3] !== 4) begin
            $display("FAIL array_methods_unique_index");
            $finish;
        end

        result = associative.find() with (item > 2);
        if (result.size() != 3
                || result[0] !== 3 || result[1] !== 3 || result[2] !== 4) begin
            $display("FAIL array_methods_associative_find");
            $finish;
        end
        result = associative.find() with (item.index() == 8);
        if (result.size() != 1 || result[0] !== 3) begin
            $display("FAIL array_methods_associative_index");
            $finish;
        end
        result = associative.find_index() with (item > 2);
        if (result.size() != 3
                || result[0] !== 4 || result[1] !== 8 || result[2] !== 10) begin
            $display("FAIL array_methods_associative_find_index");
            $finish;
        end
        result = associative.unique_index();
        if (result.size() != 4
                || result[0] !== 2 || result[1] !== 4
                || result[2] !== 6 || result[3] !== 10) begin
            $display("FAIL array_methods_associative_unique_index");
            $finish;
        end

        total = values.sum() with (item * 64'd257);
        if (total !== 32'd3341) begin
            $display("FAIL array_methods_reduction total=%0d", total);
            $finish;
        end
        total = values.sum() with (item.index());
        if (total !== 32'd10) begin
            $display("FAIL array_methods_index total=%0d", total);
            $finish;
        end

        values.sort();
        if (values[0] !== 1 || values[1] !== 2 || values[2] !== 3
                || values[3] !== 3 || values[4] !== 4) begin
            $display("FAIL array_methods_sort");
            $finish;
        end
        values.rsort();
        if (values[0] !== 4 || values[1] !== 3 || values[2] !== 3
                || values[3] !== 2 || values[4] !== 1) begin
            $display("FAIL array_methods_rsort");
            $finish;
        end
        values.sort() with (item);
        values.reverse();
        if (values[0] !== 4 || values[1] !== 3 || values[2] !== 3
                || values[3] !== 2 || values[4] !== 1) begin
            $display("FAIL array_methods_reverse");
            $finish;
        end
        values.shuffle();
        if (values.size() != 5 || values.sum() !== 32'd13) begin
            $display("FAIL array_methods_shuffle");
            $finish;
        end

        dynamic_values = new[5];
        dynamic_values[0] = 1;
        dynamic_values[1] = 3;
        dynamic_values[2] = 2;
        dynamic_values[3] = 3;
        dynamic_values[4] = 4;
        result = dynamic_values.find_first_index() with (item > 2);
        if (result.size() != 1 || result[0] !== 1) begin
            $display("FAIL array_methods_dynamic_find");
            $finish;
        end
        dynamic_values.sort() with (item);
        if (dynamic_values[0] !== 1 || dynamic_values[1] !== 2
                || dynamic_values[2] !== 3 || dynamic_values[3] !== 3
                || dynamic_values[4] !== 4) begin
            $display("FAIL array_methods_dynamic_sort");
            $finish;
        end

        $display("PASS array_methods");
        $finish;
    end
endmodule

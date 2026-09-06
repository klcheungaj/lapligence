// IEEE 1800-2009 7.8 and 7.9: string-indexed associative arrays allocate on
// write, report membership/size, traverse lexicographically, and support delete.
module tb;
    logic [7:0] values[string];
    string index;
    integer status;

    initial begin
        if (values.num() !== 0 || values.exists("alpha") !== 0 ||
            values["missing"] !== 8'hxx) begin
            $display("FAIL associative_array defaults");
            $finish;
        end

        values["gamma"] = 8'd3;
        values["alpha"] = 8'd1;
        values["beta"] = 8'd2;
        if (values.num() !== 3 || values.exists("beta") !== 1) begin
            $display("FAIL associative_array insert");
            $finish;
        end

        status = values.first(index);
        if (status !== 1 || index != "alpha") begin
            $display("FAIL associative_array first");
            $finish;
        end
        status = values.next(index);
        if (status !== 1 || index != "beta") begin
            $display("FAIL associative_array next_beta");
            $finish;
        end
        status = values.next(index);
        if (status !== 1 || index != "gamma") begin
            $display("FAIL associative_array next_gamma");
            $finish;
        end
        status = values.next(index);
        if (status !== 0 || index != "gamma") begin
            $display("FAIL associative_array traversal");
            $finish;
        end

        values.delete("beta");
        if (values.num() !== 2 || values.exists("beta") !== 0) begin
            $display("FAIL associative_array element_delete");
            $finish;
        end
        values.delete();
        if (values.num() !== 0) begin
            $display("FAIL associative_array whole_delete");
            $finish;
        end

        $display("PASS associative_array");
        $finish;
    end
endmodule

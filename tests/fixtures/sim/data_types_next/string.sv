// IEEE 1800-2009 6.16: strings resize dynamically, index from the left at
// zero, preserve arbitrary byte values, and provide deterministic core methods.
module tb;
    string value;
    string slice;
    string repeated;

    initial begin
        if (value != "" || value.len() !== 0) begin
            $display("FAIL string default");
            $finish;
        end

        value = "ab";
        value = {value, "cd"};
        if (value != "abcd" || value.len() !== 4 ||
            value[0] !== 8'h61 || value.getc(3) !== 8'h64 ||
            value.getc(20) !== 8'h00) begin
            $display("FAIL string concatenate_index");
            $finish;
        end

        value.putc(1, 8'h5a);
        value[2] = 8'h59;
        value[3] = 8'h00;
        if (value.len() !== 4 || value[0] !== 8'h61 ||
            value[1] !== 8'h5a || value[2] !== 8'h59 ||
            value.getc(3) !== 8'h00) begin
            $display("FAIL string character_write");
            $finish;
        end

        slice = value.substr(1, 2);
        repeated = {3{"xy"}};
        if (slice != "ZY" || repeated != "xyxyxy") begin
            $display("FAIL string methods_replication");
            $finish;
        end

        $display("PASS string");
        $finish;
    end
endmodule

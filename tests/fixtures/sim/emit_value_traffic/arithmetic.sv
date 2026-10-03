module tb;
    logic [127:0] acc, wide, result;
    logic [64:0] boundary;
    logic [63:0] low;
    logic signed [63:0] signed_low;
    function automatic logic [127:0] output_value(input logic [127:0] value, output logic [127:0] target);
        target = 9;
        return value + 3;
    endfunction
    initial begin
        acc = 5;
        wide = 7;
        acc <= acc * 3 + wide;
        acc = acc + 1;
        #1;
        $display("nba=%0d", acc);
        acc = 5;
        result = output_value(acc, acc);
        $display("output=%0d,%0d", result, acc);
        wide = '1;
        wide[(wide = 0) +: 1] = wide;
        $display("selector=%0d", wide);
        acc = 128'h0000000000000000000000000000000z;
        wide = acc + 128'd3;
        $display("xz=%b,%b", wide, acc[3:0]);
        low = 64'h8000000000000000;
        signed_low = $signed(low);
        boundary = 65'(low);
        $display("unsigned65=%h", boundary);
        boundary = $unsigned(65'(signed_low));
        $display("signed65=%h", boundary);
        low = 64'(boundary);
        $display("back64=%h", low);
        $finish(0);
    end
endmodule

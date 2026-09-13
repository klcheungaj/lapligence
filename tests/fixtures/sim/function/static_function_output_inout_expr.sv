module tb;
    logic [7:0] side;
    logic [7:0] side2;
    logic [7:0] packed_result;
    logic [7:0] inout_result;
    real rside;
    real real_result;

    function logic [7:0] make_inner(input logic [7:0] value,
                                    output logic [7:0] result);
        result = value + 8'd1;
        make_inner = result;
    endfunction

    function logic [7:0] make_value(input logic [7:0] value,
                                    output logic [7:0] result);
        make_value = make_inner(value, result) + 8'd1;
    endfunction

    function logic [7:0] add_value(input logic [7:0] value,
                                   inout logic [7:0] result);
        result = result + value;
        add_value = result;
    endfunction

    function real make_real(input real value, output real result);
        result = value + 1.0;
        make_real = result + 1.0;
    endfunction

    initial begin
        side = 0;
        packed_result = make_value(8'd4, side);

        side2 = 5;
        inout_result = add_value(8'd3, side2);

        rside = 1.5;
        real_result = make_real(1.5, rside);

        $display("packed=%0d side=%0d inout=%0d side2=%0d real=%0.1f rside=%0.1f",
                 packed_result, side, inout_result, side2, real_result, rside);
        $finish;
    end
endmodule

// H28 bounded VPI positive fixture.
module leaf(input logic in_value, output logic out_value);
    assign out_value = in_value;
endmodule

module tb;
    logic value;
    logic [4:0] sized_result;
    real real_result;
    wire alias_wire;

    leaf u_leaf(.in_value(value), .out_value(alias_wire));

    initial begin
        value = 1'b1;
        #1;
        $vpi_probe(value);
        sized_result = $vpi_sized(value);
        real_result = $vpi_real();
        $display("hdl=%b/%0d/%0.1f", value, sized_result, real_result);
        $finish(0);
    end
endmodule

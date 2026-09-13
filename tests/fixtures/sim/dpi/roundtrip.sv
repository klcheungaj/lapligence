// H27 DPI-C scalar import roundtrip: values, directions, aliases and qualifiers.
module tb;
    import "DPI-C" dpi_add = function int add(input int a, input int b);
    import "DPI-C" dpi_transform = function void transform(
        input int a, output int b, inout int c);
    import "DPI-C" dpi_task = task task_call(input int a, output int b);
    import "DPI-C" dpi_logic = function logic logic_id(input logic value);
    import "DPI-C" dpi_logic_io = function void logic_io(
        input logic a, output logic b, inout logic c);
    import "DPI-C" dpi_reg = function reg reg_id(input reg value);
    import "DPI-C" dpi_bit = function bit bit_id(input bit value);
    import "DPI-C" dpi_byte = function byte byte_id(input byte value);
    import "DPI-C" dpi_u64 = function longint unsigned u64_id(
        input longint unsigned value);
    import "DPI-C" dpi_real = function real real_id(input real value);
    import "DPI-C" dpi_real_io = function void real_io(
        input real a, output real b, inout real c);
    import "DPI-C" dpi_shortreal = function shortreal shortreal_id(
        input shortreal value);
    import "DPI-C" dpi_handle = function chandle handle_id(input chandle value);
    import "DPI-C" dpi_handle_io = function void handle_io(
        input chandle a, output chandle b, inout chandle c);
    import "DPI-C" dpi_string = function string string_id(input string value);
    import "DPI-C" dpi_string_io = function void string_io(
        input string input_value, output string output_value,
        inout string inout_value);
    import "DPI-C" pure function int pure_add(input int a, input int b);
    import "DPI-C" context function int context_add(input int a, input int b);

    int b;
    int c;
    int task_out;
    int sum;
    logic logic_out;
    logic logic_inout;
    reg reg_out;
    bit bit_out;
    byte byte_out;
    longint unsigned u64_out;
    real real_out;
    real real_io_out;
    real real_io_inout;
    shortreal shortreal_out;
    chandle handle_out;
    chandle handle_io_out;
    chandle handle_io_inout;
    string string_out;
    string string_io_out;
    string string_io_inout;

    initial begin
        c = -3;
        transform(7, b, c);
        task_call(8, task_out);
        logic_inout = 1'b0;
        logic_io(1'bz, logic_out, logic_inout);
        reg_out = reg_id(1'bx);
        bit_out = bit_id(1'b1);
        byte_out = byte_id(-8'sd2);
        u64_out = u64_id(64'hffff_ffff_ffff_fffd);
        real_out = real_id(1.25);
        real_io_inout = 3.0;
        real_io(1.5, real_io_out, real_io_inout);
        shortreal_out = shortreal_id(2.5);
        handle_out = handle_id(null);
        handle_io_inout = null;
        handle_io(null, handle_io_out, handle_io_inout);
        string_out = string_id("dpi-ok");
        string_io_inout = "";
        string_io("input", string_io_out, string_io_inout);
        sum = add(-2, 5) + pure_add(1, 2) + context_add(4, 5);

        $display("int=%0d,%0d task=%0d sum=%0d", b, c, task_out, sum);
        $display("logic=%b/%b/%b reg=%b bit=%b", logic_id(1'bx), logic_out,
            logic_inout, reg_out, bit_out);
        $display("byte=%0d u64=%0d", byte_out, u64_out);
        $display("real=%f io=%f/%f", real_out, real_io_out, real_io_inout);
        if (shortreal_out == 2.5)
            $display("shortreal=ok");
        else
            $display("shortreal=bad");
        $display("handle_null=%b io=%b/%b string=%s io=%s/%s", handle_out == null,
            handle_io_out == null, handle_io_inout == null, string_out,
            string_io_out, string_io_inout);
        $finish;
    end
endmodule

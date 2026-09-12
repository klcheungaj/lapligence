// llg-test-fixture: tests/fixtures/sim/partial_features/system_host_command.sv
// IEEE 1800-2009 §20.18: `$system` task/function forms use one optional
// string command and return the host command status when used as a function.
module tb;
    integer calls;
    integer status;
    integer empty_status;
    integer empty_string_status;

    function automatic string command_text();
        calls = calls + 1;
        command_text = "echo llg_system_function";
    endfunction

    initial begin
        calls = 0;
        $system("echo llg_system_task");
        $system();
        status = $system(command_text());
        empty_status = $system();
        empty_string_status = $system("");
        $display("status=%0d calls=%0d empty=%0d empty_string=%0d",
                 status, calls, empty_status, empty_string_status);
        $finish(0);
    end
endmodule

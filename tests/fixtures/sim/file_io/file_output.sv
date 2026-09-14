// llg-test-fixture: tests/fixtures/sim/file_io/file_output.sv
`timescale 1ns/1ps
module tb;
  integer fd;
  integer default_fd;
  integer owned_fd;
  integer final_fd;
  integer status;
  string default_path;
  string default_mode;
  string message;
  reg [7:0] value;

  initial begin
    value = 8'hab;
    fd = $fopen("file_output.txt", "w");
    default_fd = $fopen("default_output.txt");
    default_path = "default_output.txt";
    default_mode = "w";
    owned_fd = $fopen(default_path, default_mode);
    $fclose(default_fd);
    $fclose(owned_fd);
    final_fd = $fopen("final_output.txt", "w");
    $display("fd_is_tagged=%0d", fd[31]);
    $fdisplay(fd, "line=%0d", 7);
    $fwrite(fd, "tail=%0h", value);
    $fdisplay(fd, " fanout=%0d", 9);
    $fdisplay(32'h8000_0001, " fanout=%0d", 9);
    $fdisplayh(fd, value);
    $fflush(fd);
    $fflush();
    status = $ftell(fd);
    $display("tell=%0d", status);
    status = $fseek(fd, 0, 0);
    $display("seek=%0d", status);
    status = $ftell(fd);
    $display("rewound=%0d", status);
    $rewind(fd);
    status = $ferror(fd, message);
    $display("error=%0d message=%s eof=%0d", status, message, $feof(fd));
    $fclose(fd);
    status = $ferror(fd, message);
    $display("closed=%0d message=%s", status, message);
    $finish(0);
  end

  final begin
    $fdisplay(final_fd, "final");
  end
endmodule

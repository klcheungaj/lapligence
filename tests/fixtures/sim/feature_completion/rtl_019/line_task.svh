// Task body included by line_locations.sv.
task automatic late_error(input int id);
  #(id + 1);
  $error("late %0d", id);
endtask

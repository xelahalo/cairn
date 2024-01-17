
import matplotlib.pyplot as plt
import sys
import numpy as np

def read_input_file(file_path):
    data = []
    with open(file_path, 'r') as file:
        for line in file:
            parts = line.strip().split()
            data.append([0 if parts[0] == 'make' else 1, int(parts[1]), int(parts[2]), float(parts[3])])
    return np.array(data)

def plot_subplots(data_1, data_2, file_path):
    make_data = data_1[data_1[:, 0] == 0]
    rattle_data_1 = data_1[data_1[:, 0] == 1]
    rattle_data_2 = data_2[data_2[:, 0] == 1]

    make_means = []
    rattle_means_1 = []
    rattle_means_2 = []
    x = np.arange(0, 11)

    for i in np.unique(make_data[:, 2]):
        tmp = make_data[np.where(make_data[:, 2] == i)]
        make_means.append(np.mean(tmp[:, 3]))
    
    for i in np.unique(rattle_data_1[:, 2]):
        tmp = rattle_data_1[np.where(rattle_data_1[:, 2] == i)]
        rattle_means_1.append(np.mean(tmp[:, 3]))

    for i in np.unique(rattle_data_2[:, 2]):
        tmp = rattle_data_2[np.where(rattle_data_2[:, 2] == i)]
        rattle_means_2.append(np.mean(tmp[:, 3]))

    plt.figure(figsize=(15, 10))
    plt.title("Make vs Rattle(Cairn) vs Rattle(fsatrace)")

    make_means = np.array(make_means)[::-1]
    rattle_means_1 = np.array(rattle_means_1)[::-1]
    rattle_means_2 = np.array(rattle_means_2)[::-1]

    # make_means = make_means / np.min(make_means)
    rattle_means_1 = rattle_means_1 / make_means
    rattle_means_2 = rattle_means_2 / make_means
    make_means = make_means / make_means
    # make_means = make_means / make_means
    # rattle_means_1 = rattle_means_1 / np.min(rattle_means_1)
    # rattle_means_2 = rattle_means_2 / np.min(rattle_means_2)

    plt.plot(x, make_means, label='make')
    plt.plot(x, rattle_means_1, label='rattle (cairn)')
    plt.plot(x, rattle_means_2, label='rattle (fsatrace)')

    plt.yscale('log')
    plt.ylabel('Runtime (log scale)')
    plt.legend()
    plt.savefig(f'{file_path}.png')

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python script.py <input_file_path_1> <input_file_path_2>")
        sys.exit(1)

    file_path_1 = sys.argv[1]
    file_path_2 = sys.argv[2]
    data_1 = read_input_file(file_path_1)
    data_2 = read_input_file(file_path_2)
    plot_subplots(data_1, data_2, file_path_1)

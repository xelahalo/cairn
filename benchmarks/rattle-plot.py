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

    colors = [
        (0.00392156862745098, 0.45098039215686275, 0.6980392156862745),
        (0.8705882352941177, 0.5607843137254902, 0.0196078431372549),
        (0.00784313725490196, 0.6196078431372549, 0.45098039215686275),
        ]

    plt.plot(x, make_means, color=colors[0], label='make', linewidth=2)
    plt.plot(x, rattle_means_1, color=colors[1],  label='rattle (cairn)', marker='o', linewidth=2)
    plt.plot(x, rattle_means_2, color=colors[2], label='rattle (fsatrace)', marker='^', linewidth=2)


    plt.yscale('log')
    plt.ylabel('Average compile time (log scale)')
    plt.xlabel('Commits from HEAD')
    plt.xticks(x)
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
